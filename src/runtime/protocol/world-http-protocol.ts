import type {
  SerializedOpenWorldRequest,
  SerializedPollWorldUpdatesRequest,
  SerializedSetChunkViewRequest,
  SerializedSetPlayerInputRequest,
  SerializedWorldHostMessage,
  WorldRemoteErrorCode,
} from "./world-wire-protocol";
import { WORLD_REMOTE_PROTOCOL_VERSION } from "./world-wire-protocol";

export {
  deserializeWorldClientMessage,
  deserializeWorldHostMessages,
  serializeWorldClientMessage,
  serializeWorldHostMessages,
  type SerializedOpenWorldRequest,
  type SerializedPollWorldUpdatesRequest,
  type SerializedSetChunkViewRequest,
  type SerializedSetPlayerInputRequest,
  type SerializedWorldClientMessage,
  type SerializedWorldHostMessage,
} from "./world-wire-protocol";

export const WORLD_HTTP_PROTOCOL_VERSION = WORLD_REMOTE_PROTOCOL_VERSION;

export type WorldHttpErrorCode = WorldRemoteErrorCode;

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
