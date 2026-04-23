import type { ClientChunkCache } from "../../world/level/client-chunk-cache";
import type { WorldClient } from "../protocol/world-client";
import {
  deserializeWorldHostMessages,
  serializeWorldClientMessage,
  type OpenWorldSessionResponse,
  type SessionChunkViewResponse,
} from "../protocol/world-http-protocol";
import type { OpenWorldRequest, SetChunkViewRequest, WorldHostMessage, WorldOpenedMessage } from "../protocol/world-messages";
import { TransportWorldClient, type WorldTransport } from "./local-world-transport";

export const DEFAULT_REMOTE_WORLD_HOST_URL = "http://127.0.0.1:4173";

type FetchLike = typeof fetch;

function trimTrailingSlash(value: string): string {
  return value.endsWith("/") ? value.slice(0, -1) : value;
}

async function readErrorMessage(response: Response): Promise<string> {
  const body = await response.text();
  if (body.length === 0) {
    return `Remote world host request failed with ${response.status.toString()} ${response.statusText}`;
  }

  try {
    const parsed = JSON.parse(body) as { error?: unknown };
    if (typeof parsed.error === "string" && parsed.error.length > 0) {
      return parsed.error;
    }
  } catch {}

  return body;
}

export class RemoteWorldTransport implements WorldTransport {
  private sessionId: string | undefined;
  private readonly baseUrl: string;
  private readonly fetchImpl: FetchLike;

  public constructor(
    baseUrl: string,
    fetchImpl?: FetchLike,
  ) {
    this.baseUrl = trimTrailingSlash(baseUrl);
    this.fetchImpl = fetchImpl ?? ((input, init) => fetch(input, init));
  }

  public async openWorld(request: OpenWorldRequest): Promise<readonly WorldHostMessage[]> {
    const response = await this.fetchJson<OpenWorldSessionResponse>(
      `${this.baseUrl}/api/world/session`,
      {
        method: "POST",
        body: JSON.stringify({
          message: serializeWorldClientMessage(request),
        }),
      },
    );
    this.sessionId = response.sessionId;
    return deserializeWorldHostMessages(response.messages);
  }

  public async setChunkView(request: SetChunkViewRequest): Promise<readonly WorldHostMessage[]> {
    if (this.sessionId === undefined) {
      throw new Error("RemoteWorldTransport.setChunkView() called before openWorld()");
    }

    const response = await this.fetchJson<SessionChunkViewResponse>(
      `${this.baseUrl}/api/world/session/${encodeURIComponent(this.sessionId)}/chunk-view`,
      {
        method: "POST",
        body: JSON.stringify({
          message: serializeWorldClientMessage(request),
        }),
      },
    );
    return deserializeWorldHostMessages(response.messages);
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
      throw new Error(await readErrorMessage(response));
    }

    return await response.json() as T;
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
