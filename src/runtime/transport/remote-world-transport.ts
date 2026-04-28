import type { ClientChunkCache } from "../../world/level/client-chunk-cache";
import type { WorldClient } from "../protocol/world-client";
import { drainWorldHostMessages } from "../protocol/world-message-queue";
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
import {
  WORLD_REMOTE_PROTOCOL_VERSION,
  type WorldRemoteErrorCode,
  type WorldSocketClientFrame,
  type WorldSocketServerFrame,
} from "../protocol/world-wire-protocol";
import type {
  OpenWorldRequest,
  PollWorldUpdatesRequest,
  SetChunkViewRequest,
  SetPlayerInputRequest,
  WorldHostMessage,
  WorldOpenedMessage,
} from "../protocol/world-messages";
import { TransportWorldClient, type TransportWorldClientOptions, type WorldTransport } from "./local-world-transport";

export const DEFAULT_REMOTE_WORLD_HOST_URL = "http://127.0.0.1:4173";

type FetchLike = typeof fetch;

export interface RemoteWorldTransportOptions {
  readonly fetchImpl?: FetchLike;
  readonly sessionId?: string;
}

export interface RemoteWorldWebSocketTransportOptions {
  readonly sessionId?: string;
  readonly webSocketFactory?: (url: string) => WebSocket;
}

export class RemoteWorldTransportError extends Error {
  public constructor(
    message: string,
    public readonly code?: WorldRemoteErrorCode,
    public readonly expectedProtocolVersion?: number,
  ) {
    super(message);
  }
}

function trimTrailingSlash(value: string): string {
  return value.endsWith("/") ? value.slice(0, -1) : value;
}

function createRemoteWorldSocketUrl(baseUrl: string): string {
  const url = new URL(baseUrl);
  if (url.protocol === "http:") {
    url.protocol = "ws:";
  } else if (url.protocol === "https:") {
    url.protocol = "wss:";
  }
  if (url.protocol !== "ws:" && url.protocol !== "wss:") {
    throw new Error(`Remote world WebSocket URL must use http, https, ws, or wss, got ${url.protocol}`);
  }
  if (url.pathname === "/" || url.pathname.length === 0) {
    url.pathname = "/api/world/socket";
  }
  url.search = "";
  url.hash = "";
  return url.href;
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
  const sessionId = findSessionId(messages);
  if (sessionId !== undefined) {
    return sessionId;
  }

  throw new Error("Remote world host did not include session_state");
}

function findSessionId(messages: readonly WorldHostMessage[]): string | undefined {
  for (let index = messages.length - 1; index >= 0; index--) {
    const message = messages[index];
    if (message?.type === "session_state") {
      return message.state.sessionId;
    }
  }

  return undefined;
}

async function readWebSocketMessageData(data: unknown): Promise<string> {
  if (typeof data === "string") {
    return data;
  }
  if (data instanceof ArrayBuffer) {
    return new TextDecoder().decode(data);
  }
  if (ArrayBuffer.isView(data)) {
    return new TextDecoder().decode(data);
  }
  if (typeof Blob !== "undefined" && data instanceof Blob) {
    return await data.text();
  }

  throw new RemoteWorldTransportError("Remote world WebSocket received a non-text frame", "malformed_message");
}

export class RemoteWorldTransport implements WorldTransport {
  private sessionId: string | undefined;
  private readonly baseUrl: string;
  private readonly fetchImpl: FetchLike;
  private lastOpenWorldRequest: OpenWorldRequest | undefined;
  private lastChunkViewRequest: SetChunkViewRequest | undefined;
  private lastPlayerInputRequest: SetPlayerInputRequest | undefined;
  private closed = false;

  public constructor(
    baseUrl: string,
    options: RemoteWorldTransportOptions = {},
  ) {
    this.baseUrl = trimTrailingSlash(baseUrl);
    this.fetchImpl = options.fetchImpl ?? ((input, init) => fetch(input, init));
    this.sessionId = options.sessionId;
  }

  public supportsChunkViewDeduplication(): boolean {
    return false;
  }

  public async openWorld(request: OpenWorldRequest): Promise<readonly WorldHostMessage[]> {
    this.assertNotClosed("openWorld");
    this.lastOpenWorldRequest = request;
    this.lastChunkViewRequest = undefined;
    this.lastPlayerInputRequest = undefined;
    return await this.openWorldInternal(request, true);
  }

  public async setChunkView(request: SetChunkViewRequest): Promise<readonly WorldHostMessage[]> {
    this.assertNotClosed("setChunkView");
    if (this.sessionId === undefined) {
      throw new Error("RemoteWorldTransport.setChunkView() called before openWorld()");
    }

    this.lastChunkViewRequest = request;
    return await this.setChunkViewInternal(request, true);
  }

  public async setPlayerInput(request: SetPlayerInputRequest): Promise<readonly WorldHostMessage[]> {
    this.assertNotClosed("setPlayerInput");
    if (this.sessionId === undefined) {
      throw new Error("RemoteWorldTransport.setPlayerInput() called before openWorld()");
    }

    this.lastPlayerInputRequest = request;
    return await this.setPlayerInputInternal(request, true);
  }

  public async pollUpdates(request: PollWorldUpdatesRequest): Promise<readonly WorldHostMessage[]> {
    this.assertNotClosed("pollUpdates");
    if (this.sessionId === undefined) {
      throw new Error("RemoteWorldTransport.pollUpdates() called before openWorld()");
    }

    return await this.pollUpdatesInternal(request, true);
  }

  public getSessionId(): string | undefined {
    return this.sessionId;
  }

  public close(): void {
    this.closed = true;
    this.sessionId = undefined;
    this.lastOpenWorldRequest = undefined;
    this.lastChunkViewRequest = undefined;
    this.lastPlayerInputRequest = undefined;
  }

  private assertNotClosed(operation: string): void {
    if (this.closed) {
      throw new Error(`RemoteWorldTransport.${operation}() called after close()`);
    }
  }

  private async openWorldInternal(request: OpenWorldRequest, allowResumeFallback: boolean): Promise<readonly WorldHostMessage[]> {
    this.assertNotClosed("openWorld");
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
      this.assertNotClosed("openWorld");
      const messages = deserializeWorldHostMessages(response.messages);
      this.sessionId = extractSessionId(messages);
      return messages;
    } catch (error) {
      this.assertNotClosed("openWorld");
      if (allowResumeFallback && error instanceof RemoteWorldTransportError && error.code === "unknown_session" && this.sessionId !== undefined) {
        this.sessionId = undefined;
        return await this.openWorldInternal(request, false);
      }

      throw error;
    }
  }

  private async setChunkViewInternal(request: SetChunkViewRequest, allowReconnect: boolean): Promise<readonly WorldHostMessage[]> {
    this.assertNotClosed("setChunkView");
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
      this.assertNotClosed("setChunkView");
      const messages = deserializeWorldHostMessages(response.messages);
      this.sessionId = extractSessionId(messages);
      return messages;
    } catch (error) {
      this.assertNotClosed("setChunkView");
      if (allowReconnect && error instanceof RemoteWorldTransportError && error.code === "unknown_session" && this.lastOpenWorldRequest !== undefined) {
        return await this.restoreSession(false);
      }

      throw error;
    }
  }

  private async setPlayerInputInternal(request: SetPlayerInputRequest, allowReconnect: boolean): Promise<readonly WorldHostMessage[]> {
    this.assertNotClosed("setPlayerInput");
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
      this.assertNotClosed("setPlayerInput");
      const messages = deserializeWorldHostMessages(response.messages);
      this.sessionId = extractSessionId(messages);
      return messages;
    } catch (error) {
      this.assertNotClosed("setPlayerInput");
      if (allowReconnect && error instanceof RemoteWorldTransportError && error.code === "unknown_session" && this.lastOpenWorldRequest !== undefined) {
        return await this.restoreSession(false);
      }

      throw error;
    }
  }

  private async pollUpdatesInternal(request: PollWorldUpdatesRequest, allowReconnect: boolean): Promise<readonly WorldHostMessage[]> {
    this.assertNotClosed("pollUpdates");
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
      this.assertNotClosed("pollUpdates");
      return deserializeWorldHostMessages(response.messages);
    } catch (error) {
      this.assertNotClosed("pollUpdates");
      if (allowReconnect && error instanceof RemoteWorldTransportError && error.code === "unknown_session" && this.lastOpenWorldRequest !== undefined) {
        return await this.restoreSession(false);
      }

      throw error;
    }
  }

  private async restoreSession(allowResumeFallback: boolean): Promise<readonly WorldHostMessage[]> {
    this.assertNotClosed("restoreSession");
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

interface PendingSocketRequest {
  readonly resolve: (messages: readonly WorldHostMessage[]) => void;
  readonly reject: (error: Error) => void;
}

const SOCKET_CONNECTING = 0;
const SOCKET_OPEN = 1;
const SOCKET_CLOSING = 2;
const SOCKET_CLOSED = 3;

export class RemoteWorldWebSocketTransport implements WorldTransport {
  private readonly socketUrl: string;
  private readonly webSocketFactory: (url: string) => WebSocket;
  private socket: WebSocket | undefined;
  private openPromise: Promise<void> | undefined;
  private sessionId: string | undefined;
  private lastOpenWorldRequest: OpenWorldRequest | undefined;
  private lastChunkViewRequest: SetChunkViewRequest | undefined;
  private lastPlayerInputRequest: SetPlayerInputRequest | undefined;
  private nextRequestId = 1;
  private closed = false;
  private readonly pendingRequests = new Map<number, PendingSocketRequest>();
  private pendingPushMessages: WorldHostMessage[] = [];

  public constructor(
    baseUrl: string,
    options: RemoteWorldWebSocketTransportOptions = {},
  ) {
    this.socketUrl = createRemoteWorldSocketUrl(baseUrl);
    this.webSocketFactory = options.webSocketFactory ?? ((url) => new WebSocket(url));
    this.sessionId = options.sessionId;
  }

  public supportsChunkViewDeduplication(): boolean {
    return false;
  }

  public async openWorld(request: OpenWorldRequest): Promise<readonly WorldHostMessage[]> {
    this.assertNotClosed("openWorld");
    this.lastOpenWorldRequest = request;
    this.lastChunkViewRequest = undefined;
    this.lastPlayerInputRequest = undefined;
    this.pendingPushMessages = [];
    return await this.openWorldInternal(request, true);
  }

  public async setChunkView(request: SetChunkViewRequest): Promise<readonly WorldHostMessage[]> {
    this.assertNotClosed("setChunkView");
    if (this.sessionId === undefined) {
      throw new Error("RemoteWorldWebSocketTransport.setChunkView() called before openWorld()");
    }

    this.lastChunkViewRequest = request;
    return await this.setChunkViewInternal(request, true);
  }

  public async setPlayerInput(request: SetPlayerInputRequest): Promise<readonly WorldHostMessage[]> {
    this.assertNotClosed("setPlayerInput");
    if (this.sessionId === undefined) {
      throw new Error("RemoteWorldWebSocketTransport.setPlayerInput() called before openWorld()");
    }

    this.lastPlayerInputRequest = request;
    return await this.setPlayerInputInternal(request, true);
  }

  public async pollUpdates(request: PollWorldUpdatesRequest): Promise<readonly WorldHostMessage[]> {
    this.assertNotClosed("pollUpdates");
    if (this.sessionId === undefined) {
      throw new Error("RemoteWorldWebSocketTransport.pollUpdates() called before openWorld()");
    }

    const drained = drainWorldHostMessages(this.pendingPushMessages, request.maxMessages);
    this.pendingPushMessages = drained.remaining;
    return drained.messages;
  }

  public getSessionId(): string | undefined {
    return this.sessionId;
  }

  public close(): void {
    this.closed = true;
    this.sessionId = undefined;
    this.lastOpenWorldRequest = undefined;
    this.lastChunkViewRequest = undefined;
    this.lastPlayerInputRequest = undefined;
    this.pendingPushMessages = [];
    this.rejectPendingRequests(new Error("RemoteWorldWebSocketTransport closed"));
    const socket = this.socket;
    this.socket = undefined;
    this.openPromise = undefined;
    if (socket !== undefined && socket.readyState !== SOCKET_CLOSED && socket.readyState !== SOCKET_CLOSING) {
      socket.close();
    }
  }

  private assertNotClosed(operation: string): void {
    if (this.closed) {
      throw new Error(`RemoteWorldWebSocketTransport.${operation}() called after close()`);
    }
  }

  private async openWorldInternal(request: OpenWorldRequest, allowResumeFallback: boolean): Promise<readonly WorldHostMessage[]> {
    this.assertNotClosed("openWorld");
    try {
      const messages = await this.sendRequest(request, this.sessionId);
      this.assertNotClosed("openWorld");
      this.sessionId = extractSessionId(messages);
      return messages;
    } catch (error) {
      this.assertNotClosed("openWorld");
      if (allowResumeFallback && error instanceof RemoteWorldTransportError && error.code === "unknown_session" && this.sessionId !== undefined) {
        this.sessionId = undefined;
        return await this.openWorldInternal(request, false);
      }

      throw error;
    }
  }

  private async setChunkViewInternal(request: SetChunkViewRequest, allowReconnect: boolean): Promise<readonly WorldHostMessage[]> {
    this.assertNotClosed("setChunkView");
    try {
      const messages = await this.sendRequest(request);
      this.assertNotClosed("setChunkView");
      this.sessionId = extractSessionId(messages);
      return messages;
    } catch (error) {
      this.assertNotClosed("setChunkView");
      if (allowReconnect && error instanceof RemoteWorldTransportError && error.code === "unknown_session" && this.lastOpenWorldRequest !== undefined) {
        return await this.restoreSession(false);
      }

      throw error;
    }
  }

  private async setPlayerInputInternal(request: SetPlayerInputRequest, allowReconnect: boolean): Promise<readonly WorldHostMessage[]> {
    this.assertNotClosed("setPlayerInput");
    try {
      const messages = await this.sendRequest(request);
      this.assertNotClosed("setPlayerInput");
      this.sessionId = extractSessionId(messages);
      return messages;
    } catch (error) {
      this.assertNotClosed("setPlayerInput");
      if (allowReconnect && error instanceof RemoteWorldTransportError && error.code === "unknown_session" && this.lastOpenWorldRequest !== undefined) {
        return await this.restoreSession(false);
      }

      throw error;
    }
  }

  private async restoreSession(allowResumeFallback: boolean): Promise<readonly WorldHostMessage[]> {
    this.assertNotClosed("restoreSession");
    if (this.lastOpenWorldRequest === undefined) {
      throw new Error("Remote world WebSocket transport could not restore a missing session before openWorld()");
    }

    this.sessionId = undefined;
    this.pendingPushMessages = [];
    const messages: WorldHostMessage[] = [...await this.openWorldInternal(this.lastOpenWorldRequest, allowResumeFallback)];
    if (this.lastChunkViewRequest !== undefined) {
      messages.push(...await this.setChunkViewInternal(this.lastChunkViewRequest, false));
    }
    if (this.lastPlayerInputRequest !== undefined) {
      messages.push(...await this.setPlayerInputInternal(this.lastPlayerInputRequest, false));
    }
    return messages;
  }

  private async sendRequest(
    request: OpenWorldRequest | SetChunkViewRequest | SetPlayerInputRequest | PollWorldUpdatesRequest,
    resumeSessionId?: string,
  ): Promise<readonly WorldHostMessage[]> {
    await this.ensureSocketOpen();
    this.assertNotClosed("sendRequest");
    const socket = this.socket;
    if (socket === undefined || socket.readyState !== SOCKET_OPEN) {
      throw new RemoteWorldTransportError("Remote world WebSocket is not open");
    }

    const requestId = this.nextRequestId++;
    const frame: WorldSocketClientFrame = {
      protocolVersion: WORLD_REMOTE_PROTOCOL_VERSION,
      kind: "request",
      requestId,
      ...(resumeSessionId === undefined ? {} : { resumeSessionId }),
      message: serializeWorldClientMessage(request),
    };
    const response = new Promise<readonly WorldHostMessage[]>((resolve, reject) => {
      this.pendingRequests.set(requestId, { resolve, reject });
    });

    try {
      socket.send(JSON.stringify(frame));
    } catch (error) {
      this.pendingRequests.delete(requestId);
      throw error;
    }

    return await response;
  }

  private async ensureSocketOpen(): Promise<void> {
    this.assertNotClosed("ensureSocketOpen");
    if (this.socket !== undefined && this.socket.readyState === SOCKET_OPEN) {
      return;
    }
    if (this.socket !== undefined && this.socket.readyState === SOCKET_CONNECTING && this.openPromise !== undefined) {
      await this.openPromise;
      return;
    }

    this.socket = undefined;
    this.openPromise = this.createSocketOpenPromise();
    await this.openPromise;
  }

  private createSocketOpenPromise(): Promise<void> {
    const socket = this.webSocketFactory(this.socketUrl);
    this.socket = socket;
    return new Promise<void>((resolve, reject) => {
      let settled = false;
      const settleReject = (error: Error): void => {
        if (settled) {
          return;
        }
        settled = true;
        reject(error);
      };
      const settleResolve = (): void => {
        if (settled) {
          return;
        }
        settled = true;
        resolve();
      };

      socket.addEventListener("open", () => {
        settleResolve();
      });
      socket.addEventListener("message", (event) => {
        void this.handleSocketMessage(event.data);
      });
      socket.addEventListener("error", () => {
        settleReject(new RemoteWorldTransportError("Remote world WebSocket connection failed"));
      });
      socket.addEventListener("close", () => {
        if (!this.closed && this.socket === socket) {
          this.socket = undefined;
          this.openPromise = undefined;
        }
        settleReject(new RemoteWorldTransportError("Remote world WebSocket connection closed"));
        this.rejectPendingRequests(new RemoteWorldTransportError("Remote world WebSocket connection closed"));
      });
    });
  }

  private async handleSocketMessage(data: unknown): Promise<void> {
    try {
      const frame = JSON.parse(await readWebSocketMessageData(data)) as WorldSocketServerFrame;
      this.handleSocketFrame(frame);
    } catch (error) {
      this.rejectPendingRequests(error instanceof Error ? error : new Error(String(error)));
    }
  }

  private handleSocketFrame(frame: WorldSocketServerFrame): void {
    if (frame.protocolVersion !== WORLD_REMOTE_PROTOCOL_VERSION) {
      this.rejectPendingRequests(new RemoteWorldTransportError(
        `Remote world WebSocket protocol version ${String(frame.protocolVersion)} did not match client version ${WORLD_REMOTE_PROTOCOL_VERSION.toString()}`,
        "protocol_version_mismatch",
        typeof frame.protocolVersion === "number" ? frame.protocolVersion : undefined,
      ));
      return;
    }

    switch (frame.kind) {
      case "response": {
        const pending = this.pendingRequests.get(frame.requestId);
        if (pending === undefined) {
          return;
        }
        this.pendingRequests.delete(frame.requestId);
        const messages = deserializeWorldHostMessages(frame.messages);
        this.updateSessionId(messages);
        pending.resolve(messages);
        break;
      }
      case "push": {
        const messages = deserializeWorldHostMessages(frame.messages);
        this.updateSessionId(messages);
        this.enqueuePushMessages(messages);
        this.sendAck(frame.sequence);
        break;
      }
      case "error": {
        const error = new RemoteWorldTransportError(frame.message, frame.code, frame.expectedProtocolVersion);
        if (frame.requestId !== undefined) {
          const pending = this.pendingRequests.get(frame.requestId);
          if (pending !== undefined) {
            this.pendingRequests.delete(frame.requestId);
            pending.reject(error);
            return;
          }
        }
        break;
      }
    }
  }

  private updateSessionId(messages: readonly WorldHostMessage[]): void {
    const nextSessionId = findSessionId(messages);
    if (nextSessionId !== undefined) {
      this.sessionId = nextSessionId;
    }
  }

  private enqueuePushMessages(messages: readonly WorldHostMessage[]): void {
    for (const message of messages) {
      switch (message.type) {
        case "world_progress":
        case "world_perf":
        case "player_state":
          this.pendingPushMessages = this.pendingPushMessages.filter((pending) => pending.type !== message.type);
          break;
        case "entity_snapshot":
          this.pendingPushMessages = this.pendingPushMessages.filter((pending) =>
            (pending.type !== "entity_snapshot" || pending.entity.id !== message.entity.id)
            && (pending.type !== "entity_update" || pending.update.id !== message.entity.id)
            && (pending.type !== "entity_remove" || pending.entityId !== message.entity.id)
          );
          break;
        case "entity_update":
          this.pendingPushMessages = this.pendingPushMessages.filter((pending) =>
            pending.type !== "entity_update" || pending.update.id !== message.update.id
          );
          break;
        case "entity_remove":
          this.pendingPushMessages = this.pendingPushMessages.filter((pending) =>
            (pending.type !== "entity_snapshot" || pending.entity.id !== message.entityId)
            && (pending.type !== "entity_update" || pending.update.id !== message.entityId)
            && (pending.type !== "entity_remove" || pending.entityId !== message.entityId)
          );
          break;
        case "world_opened":
        case "session_state":
        case "chunk_snapshot":
        case "chunk_light_delta":
        case "chunk_unload":
        case "world_error":
          break;
      }
      this.pendingPushMessages.push(message);
    }
  }

  private sendAck(sequence: number): void {
    if (this.socket === undefined || this.socket.readyState !== SOCKET_OPEN) {
      return;
    }

    const frame: WorldSocketClientFrame = {
      protocolVersion: WORLD_REMOTE_PROTOCOL_VERSION,
      kind: "ack",
      receivedSequence: sequence,
    };
    this.socket.send(JSON.stringify(frame));
  }

  private rejectPendingRequests(error: Error): void {
    for (const pending of this.pendingRequests.values()) {
      pending.reject(error);
    }
    this.pendingRequests.clear();
  }
}

export class RemoteWorldClient extends TransportWorldClient implements WorldClient {
  public constructor(
    transport: WorldTransport,
    levelFactory: (worldOpened: WorldOpenedMessage) => ClientChunkCache,
    options?: TransportWorldClientOptions,
  ) {
    super(transport, levelFactory, {
      ...options,
      pollUpdateMaxMessages: options?.pollUpdateMaxMessages ?? 2,
    });
  }
}
