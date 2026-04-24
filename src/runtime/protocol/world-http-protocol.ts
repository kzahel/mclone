import type {
  ChunkLightDeltaMessage,
  ChunkSnapshotMessage,
  ChunkUnloadMessage,
  PlayerStateMessage,
  SessionStateMessage,
  WorldErrorMessage,
  OpenWorldPreset,
  PollWorldUpdatesRequest,
  SetChunkViewRequest,
  SetPlayerInputRequest,
  WorldOpenedMessage,
  WorldClientMessage,
  WorldEngineConfig,
  WorldHostMessage,
  WorldProgressMessage,
} from "./world-messages";
import {
  deserializePackedChunkLightDelta,
  deserializePackedChunkSnapshot,
  serializePackedChunkLightDelta,
  serializePackedChunkSnapshot,
  type SerializedPackedChunkLightDelta,
  type SerializedPackedChunkSnapshot,
} from "./packed-chunk-wire";

export const WORLD_HTTP_PROTOCOL_VERSION = 5;

export type WorldHttpErrorCode =
  | "protocol_version_mismatch"
  | "unknown_session"
  | "world_request_mismatch";

export interface SerializedOpenWorldRequest {
  readonly type: "open_world";
  readonly seed: string;
  readonly preset: OpenWorldPreset;
  readonly config?: WorldEngineConfig;
}

export type SerializedSetChunkViewRequest = SetChunkViewRequest;
export type SerializedSetPlayerInputRequest = SetPlayerInputRequest;
export type SerializedPollWorldUpdatesRequest = PollWorldUpdatesRequest;
export type SerializedWorldClientMessage =
  | SerializedOpenWorldRequest
  | SerializedSetChunkViewRequest
  | SerializedSetPlayerInputRequest
  | SerializedPollWorldUpdatesRequest;

export interface SerializedChunkSnapshotMessage {
  readonly type: "chunk_snapshot";
  readonly snapshot: SerializedPackedChunkSnapshot;
}

export interface SerializedChunkLightDeltaMessage {
  readonly type: "chunk_light_delta";
  readonly chunkX: number;
  readonly chunkZ: number;
  readonly light: SerializedPackedChunkLightDelta;
}

export type SerializedWorldHostMessage =
  | WorldOpenedMessage
  | SessionStateMessage
  | PlayerStateMessage
  | SerializedChunkSnapshotMessage
  | SerializedChunkLightDeltaMessage
  | ChunkUnloadMessage
  | WorldProgressMessage
  | WorldErrorMessage;

export interface OpenWorldSessionRequest {
  readonly protocolVersion: number;
  readonly resumeSessionId?: string;
  readonly message: SerializedOpenWorldRequest;
}

export interface OpenWorldSessionResponse {
  readonly protocolVersion: number;
  readonly messages: readonly SerializedWorldHostMessage[];
}

export interface SessionChunkViewRequest {
  readonly protocolVersion: number;
  readonly message: SerializedSetChunkViewRequest;
}

export interface SessionChunkViewResponse {
  readonly protocolVersion: number;
  readonly messages: readonly SerializedWorldHostMessage[];
}

export interface SessionPlayerInputRequest {
  readonly protocolVersion: number;
  readonly message: SerializedSetPlayerInputRequest;
}

export interface SessionPlayerInputResponse {
  readonly protocolVersion: number;
  readonly messages: readonly SerializedWorldHostMessage[];
}

export interface SessionPollUpdatesRequest {
  readonly protocolVersion: number;
  readonly message: SerializedPollWorldUpdatesRequest;
}

export interface SessionPollUpdatesResponse {
  readonly protocolVersion: number;
  readonly messages: readonly SerializedWorldHostMessage[];
}

export interface WorldHttpErrorResponse {
  readonly protocolVersion: number;
  readonly error: {
    readonly code: WorldHttpErrorCode;
    readonly message: string;
    readonly expectedProtocolVersion?: number;
  };
}

export function serializeWorldClientMessage(message: WorldClientMessage): SerializedWorldClientMessage {
  switch (message.type) {
    case "open_world":
      return {
        type: "open_world",
        seed: message.seed.toString(),
        preset: message.preset,
        ...(message.config === undefined ? {} : { config: message.config }),
      };
    case "set_chunk_view":
      return message;
    case "set_player_input":
      return message;
    case "poll_world_updates":
      return message;
  }
}

export function deserializeWorldClientMessage(message: SerializedWorldClientMessage): WorldClientMessage {
  switch (message.type) {
    case "open_world":
      return {
        type: "open_world",
        seed: BigInt(message.seed),
        preset: message.preset,
        ...(message.config === undefined ? {} : { config: message.config }),
      };
    case "set_chunk_view":
      return message;
    case "set_player_input":
      return message;
    case "poll_world_updates":
      return message;
  }
}

export function serializeWorldHostMessages(messages: readonly WorldHostMessage[]): readonly SerializedWorldHostMessage[] {
  return messages.map((message) => {
    switch (message.type) {
      case "world_opened":
      case "session_state":
      case "player_state":
      case "chunk_unload":
      case "world_progress":
      case "world_error":
        return message;
      case "chunk_snapshot":
        return {
          type: "chunk_snapshot",
          snapshot: serializePackedChunkSnapshot(message.snapshot),
        };
      case "chunk_light_delta":
        return {
          type: "chunk_light_delta",
          chunkX: message.chunkX,
          chunkZ: message.chunkZ,
          light: serializePackedChunkLightDelta(message.light),
        };
    }
  });
}

export function deserializeWorldHostMessages(messages: readonly SerializedWorldHostMessage[]): readonly WorldHostMessage[] {
  return messages.map((message) => {
    switch (message.type) {
      case "world_opened":
      case "session_state":
      case "player_state":
      case "chunk_unload":
      case "world_progress":
      case "world_error":
        return message;
      case "chunk_snapshot":
        return {
          type: "chunk_snapshot",
          snapshot: deserializePackedChunkSnapshot(message.snapshot),
        } satisfies ChunkSnapshotMessage;
      case "chunk_light_delta":
        return {
          type: "chunk_light_delta",
          chunkX: message.chunkX,
          chunkZ: message.chunkZ,
          light: deserializePackedChunkLightDelta(message.light),
        } satisfies ChunkLightDeltaMessage;
    }
  });
}
