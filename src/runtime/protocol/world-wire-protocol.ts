import type {
  ChunkLightDeltaMessage,
  ChunkSnapshotMessage,
  ChunkUnloadMessage,
  EntitySnapshotMessage,
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
  WorldPerformanceMessage,
  WorldProgressMessage,
  WorldStorageMode,
} from "./world-messages";
import {
  deserializePackedChunkLightDelta,
  deserializePackedChunkSnapshot,
  serializePackedChunkLightDelta,
  serializePackedChunkSnapshot,
  type SerializedPackedChunkLightDelta,
  type SerializedPackedChunkSnapshot,
} from "./packed-chunk-wire";

export const WORLD_REMOTE_PROTOCOL_VERSION = 6;

export type WorldRemoteErrorCode =
  | "protocol_version_mismatch"
  | "unknown_session"
  | "world_request_mismatch"
  | "malformed_message"
  | "unsupported_message";

export interface SerializedOpenWorldRequest {
  readonly type: "open_world";
  readonly seed: string;
  readonly preset: OpenWorldPreset;
  readonly config?: WorldEngineConfig;
  readonly storageMode?: WorldStorageMode;
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
  | EntitySnapshotMessage
  | SerializedChunkSnapshotMessage
  | SerializedChunkLightDeltaMessage
  | ChunkUnloadMessage
  | WorldProgressMessage
  | WorldPerformanceMessage
  | WorldErrorMessage;

export type WorldSocketClientFrame =
  | {
    readonly protocolVersion: number;
    readonly kind: "request";
    readonly requestId: number;
    readonly resumeSessionId?: string;
    readonly message: SerializedWorldClientMessage;
  }
  | {
    readonly protocolVersion: number;
    readonly kind: "ack";
    readonly receivedSequence: number;
  };

export type WorldSocketServerFrame =
  | {
    readonly protocolVersion: number;
    readonly kind: "response";
    readonly requestId: number;
    readonly messages: readonly SerializedWorldHostMessage[];
  }
  | {
    readonly protocolVersion: number;
    readonly kind: "push";
    readonly sequence: number;
    readonly messages: readonly SerializedWorldHostMessage[];
  }
  | {
    readonly protocolVersion: number;
    readonly kind: "error";
    readonly requestId?: number;
    readonly code: WorldRemoteErrorCode;
    readonly message: string;
    readonly expectedProtocolVersion?: number;
  };

export function serializeWorldClientMessage(message: WorldClientMessage): SerializedWorldClientMessage {
  switch (message.type) {
    case "open_world":
      return {
        type: "open_world",
        seed: message.seed.toString(),
        preset: message.preset,
        ...(message.config === undefined ? {} : { config: message.config }),
        ...(message.storageMode === undefined ? {} : { storageMode: message.storageMode }),
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
        ...(message.storageMode === undefined ? {} : { storageMode: message.storageMode }),
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
      case "entity_snapshot":
      case "chunk_unload":
      case "world_progress":
      case "world_perf":
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
      case "entity_snapshot":
      case "chunk_unload":
      case "world_progress":
      case "world_perf":
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
