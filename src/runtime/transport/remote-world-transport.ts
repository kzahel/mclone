import type { ClientChunkCache } from "../../world/level/client-chunk-cache";
import type { WorldClient } from "../protocol/world-client";
import {
  deserializeWorldHostMessages,
  serializeWorldClientMessage,
  WORLD_HTTP_PROTOCOL_VERSION,
  type OpenWorldSessionResponse,
  type SessionPlayerInputResponse,
  type SessionPollUpdatesResponse,
  type SessionChunkViewResponse,
  type WorldHttpErrorCode,
  type WorldHttpErrorResponse,
} from "../protocol/world-http-protocol";
import type {
  OpenWorldRequest,
  PollWorldUpdatesRequest,
  SetChunkViewRequest,
  SetPlayerInputRequest,
  WorldHostMessage,
  WorldOpenedMessage,
} from "../protocol/world-messages";
import { TransportWorldClient, type WorldTransport } from "./local-world-transport";

export const DEFAULT_REMOTE_WORLD_HOST_URL = "http://127.0.0.1:4173";

type FetchLike = typeof fetch;

export interface RemoteWorldTransportOptions {
  readonly fetchImpl?: FetchLike;
  readonly sessionId?: string;
}

export class RemoteWorldTransportError extends Error {
  public constructor(
    message: string,
    public readonly code?: WorldHttpErrorCode,
    public readonly expectedProtocolVersion?: number,
  ) {
    super(message);
  }
}

function trimTrailingSlash(value: string): string {
  return value.endsWith("/") ? value.slice(0, -1) : value;
}

async function readError(response: Response): Promise<RemoteWorldTransportError> {
  const body = await response.text();
  if (body.length === 0) {
    return new RemoteWorldTransportError(`Remote world host request failed with ${response.status.toString()} ${response.statusText}`);
  }

  try {
    const parsed = JSON.parse(body) as WorldHttpErrorResponse | { error?: unknown };
    if (typeof parsed === "object" && parsed !== null && "error" in parsed && typeof parsed.error === "object" && parsed.error !== null) {
      const errorBody = parsed.error as Record<string, unknown>;
      return new RemoteWorldTransportError(
        typeof errorBody.message === "string" ? errorBody.message : body,
        typeof errorBody.code === "string" ? errorBody.code as WorldHttpErrorCode : undefined,
        typeof errorBody.expectedProtocolVersion === "number" ? errorBody.expectedProtocolVersion : undefined,
      );
    }

    if (typeof parsed.error === "string" && parsed.error.length > 0) {
      return new RemoteWorldTransportError(parsed.error);
    }
  } catch {}

  return new RemoteWorldTransportError(body);
}

function extractSessionId(messages: readonly WorldHostMessage[]): string {
  for (let index = messages.length - 1; index >= 0; index--) {
    const message = messages[index];
    if (message?.type === "session_state") {
      return message.state.sessionId;
    }
  }

  throw new Error("Remote world host did not include session_state");
}

export class RemoteWorldTransport implements WorldTransport {
  private sessionId: string | undefined;
  private readonly baseUrl: string;
  private readonly fetchImpl: FetchLike;
  private lastOpenWorldRequest: OpenWorldRequest | undefined;
  private lastChunkViewRequest: SetChunkViewRequest | undefined;
  private lastPlayerInputRequest: SetPlayerInputRequest | undefined;

  public constructor(
    baseUrl: string,
    options: RemoteWorldTransportOptions = {},
  ) {
    this.baseUrl = trimTrailingSlash(baseUrl);
    this.fetchImpl = options.fetchImpl ?? ((input, init) => fetch(input, init));
    this.sessionId = options.sessionId;
  }

  public async openWorld(request: OpenWorldRequest): Promise<readonly WorldHostMessage[]> {
    this.lastOpenWorldRequest = request;
    this.lastChunkViewRequest = undefined;
    this.lastPlayerInputRequest = undefined;
    return await this.openWorldInternal(request, true);
  }

  public async setChunkView(request: SetChunkViewRequest): Promise<readonly WorldHostMessage[]> {
    if (this.sessionId === undefined) {
      throw new Error("RemoteWorldTransport.setChunkView() called before openWorld()");
    }

    this.lastChunkViewRequest = request;
    return await this.setChunkViewInternal(request, true);
  }

  public async setPlayerInput(request: SetPlayerInputRequest): Promise<readonly WorldHostMessage[]> {
    if (this.sessionId === undefined) {
      throw new Error("RemoteWorldTransport.setPlayerInput() called before openWorld()");
    }

    this.lastPlayerInputRequest = request;
    return await this.setPlayerInputInternal(request, true);
  }

  public async pollUpdates(request: PollWorldUpdatesRequest): Promise<readonly WorldHostMessage[]> {
    if (this.sessionId === undefined) {
      throw new Error("RemoteWorldTransport.pollUpdates() called before openWorld()");
    }

    return await this.pollUpdatesInternal(request, true);
  }

  public getSessionId(): string | undefined {
    return this.sessionId;
  }

  private async openWorldInternal(request: OpenWorldRequest, allowResumeFallback: boolean): Promise<readonly WorldHostMessage[]> {
    try {
      const response = await this.fetchJson<OpenWorldSessionResponse>(
        `${this.baseUrl}/api/world/session`,
        {
          method: "POST",
          body: JSON.stringify({
            protocolVersion: WORLD_HTTP_PROTOCOL_VERSION,
            resumeSessionId: this.sessionId,
            message: serializeWorldClientMessage(request),
          }),
        },
      );
      const messages = deserializeWorldHostMessages(response.messages);
      this.sessionId = extractSessionId(messages);
      return messages;
    } catch (error) {
      if (allowResumeFallback && error instanceof RemoteWorldTransportError && error.code === "unknown_session" && this.sessionId !== undefined) {
        this.sessionId = undefined;
        return await this.openWorldInternal(request, false);
      }

      throw error;
    }
  }

  private async setChunkViewInternal(request: SetChunkViewRequest, allowReconnect: boolean): Promise<readonly WorldHostMessage[]> {
    try {
      const response = await this.fetchJson<SessionChunkViewResponse>(
        `${this.baseUrl}/api/world/session/${encodeURIComponent(this.sessionId!)}/chunk-view`,
        {
          method: "POST",
          body: JSON.stringify({
            protocolVersion: WORLD_HTTP_PROTOCOL_VERSION,
            message: serializeWorldClientMessage(request),
          }),
        },
      );
      const messages = deserializeWorldHostMessages(response.messages);
      this.sessionId = extractSessionId(messages);
      return messages;
    } catch (error) {
      if (allowReconnect && error instanceof RemoteWorldTransportError && error.code === "unknown_session" && this.lastOpenWorldRequest !== undefined) {
        return await this.restoreSession(false);
      }

      throw error;
    }
  }

  private async setPlayerInputInternal(request: SetPlayerInputRequest, allowReconnect: boolean): Promise<readonly WorldHostMessage[]> {
    try {
      const response = await this.fetchJson<SessionPlayerInputResponse>(
        `${this.baseUrl}/api/world/session/${encodeURIComponent(this.sessionId!)}/player-input`,
        {
          method: "POST",
          body: JSON.stringify({
            protocolVersion: WORLD_HTTP_PROTOCOL_VERSION,
            message: serializeWorldClientMessage(request),
          }),
        },
      );
      const messages = deserializeWorldHostMessages(response.messages);
      this.sessionId = extractSessionId(messages);
      return messages;
    } catch (error) {
      if (allowReconnect && error instanceof RemoteWorldTransportError && error.code === "unknown_session" && this.lastOpenWorldRequest !== undefined) {
        return await this.restoreSession(false);
      }

      throw error;
    }
  }

  private async pollUpdatesInternal(request: PollWorldUpdatesRequest, allowReconnect: boolean): Promise<readonly WorldHostMessage[]> {
    try {
      const response = await this.fetchJson<SessionPollUpdatesResponse>(
        `${this.baseUrl}/api/world/session/${encodeURIComponent(this.sessionId!)}/updates/poll`,
        {
          method: "POST",
          body: JSON.stringify({
            protocolVersion: WORLD_HTTP_PROTOCOL_VERSION,
            message: serializeWorldClientMessage(request),
          }),
        },
      );
      return deserializeWorldHostMessages(response.messages);
    } catch (error) {
      if (allowReconnect && error instanceof RemoteWorldTransportError && error.code === "unknown_session" && this.lastOpenWorldRequest !== undefined) {
        return await this.restoreSession(false);
      }

      throw error;
    }
  }

  private async restoreSession(allowResumeFallback: boolean): Promise<readonly WorldHostMessage[]> {
    if (this.lastOpenWorldRequest === undefined) {
      throw new Error("Remote world transport could not restore a missing session before openWorld()");
    }

    this.sessionId = undefined;
    const messages: WorldHostMessage[] = [...await this.openWorldInternal(this.lastOpenWorldRequest, allowResumeFallback)];
    if (this.lastChunkViewRequest !== undefined) {
      messages.push(...await this.setChunkViewInternal(this.lastChunkViewRequest, false));
    }
    if (this.lastPlayerInputRequest !== undefined) {
      messages.push(...await this.setPlayerInputInternal(this.lastPlayerInputRequest, false));
    }
    return messages;
  }

  private async fetchJson<T>(url: string, init: RequestInit): Promise<T> {
    const response = await this.fetchImpl(url, {
      ...init,
      headers: {
        "content-type": "application/json",
        ...(init.headers ?? {}),
      },
    });
    if (!response.ok) {
      throw await readError(response);
    }

    const body = await response.json() as { protocolVersion?: unknown };
    if (body.protocolVersion !== WORLD_HTTP_PROTOCOL_VERSION) {
      throw new RemoteWorldTransportError(
        `Remote world host protocol version ${String(body.protocolVersion)} did not match client version ${WORLD_HTTP_PROTOCOL_VERSION.toString()}`,
        "protocol_version_mismatch",
        typeof body.protocolVersion === "number" ? body.protocolVersion : undefined,
      );
    }

    return body as T;
  }
}

export class RemoteWorldClient extends TransportWorldClient implements WorldClient {
  public constructor(
    transport: RemoteWorldTransport,
    levelFactory: (worldOpened: WorldOpenedMessage) => ClientChunkCache,
  ) {
    super(transport, levelFactory);
  }
}
