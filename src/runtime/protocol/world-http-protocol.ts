import type {
  OpenWorldPreset,
  PollWorldUpdatesRequest,
  SetChunkViewRequest,
  SetPlayerInputRequest,
  WorldClientMessage,
  WorldHostMessage,
} from "./world-messages";

export const WORLD_HTTP_PROTOCOL_VERSION = 2;

export type WorldHttpErrorCode =
  | "protocol_version_mismatch"
  | "unknown_session"
  | "world_request_mismatch";

export interface SerializedOpenWorldRequest {
  readonly type: "open_world";
  readonly seed: string;
  readonly preset: OpenWorldPreset;
}

export type SerializedSetChunkViewRequest = SetChunkViewRequest;
export type SerializedSetPlayerInputRequest = SetPlayerInputRequest;
export type SerializedPollWorldUpdatesRequest = PollWorldUpdatesRequest;
export type SerializedWorldClientMessage =
  | SerializedOpenWorldRequest
  | SerializedSetChunkViewRequest
  | SerializedSetPlayerInputRequest
  | SerializedPollWorldUpdatesRequest;
export type SerializedWorldHostMessage = WorldHostMessage;

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
  return messages;
}

export function deserializeWorldHostMessages(messages: readonly SerializedWorldHostMessage[]): readonly WorldHostMessage[] {
  return messages;
}
