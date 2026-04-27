export type BrowserWorldTransport = "worker" | "remote";

export interface BrowserWorldTransportConfig {
  readonly worldTransport: BrowserWorldTransport;
  readonly remoteWorldHostUrl?: string;
}

export const DEFAULT_DEDICATED_WORLD_SOCKET_URL = "ws://127.0.0.1:4173/api/world/socket";

const DEFAULT_DEDICATED_WORLD_SOCKET_PATH = "/api/world/socket";
const ABSOLUTE_URL_SCHEME = /^[A-Za-z][A-Za-z0-9+.-]*:\/\//;

export function readBrowserWorldTransportConfig(url: URL): BrowserWorldTransportConfig {
  const worldAuthority = url.searchParams.get("worldAuthority");
  if (worldAuthority !== null) {
    return readWorldAuthorityTransportConfig(url, worldAuthority);
  }

  const worldTransport = url.searchParams.get("worldTransport");
  if (worldTransport === "remote") {
    const remoteWorldHostUrl = url.searchParams.get("worldHostUrl");
    return {
      worldTransport: "remote",
      remoteWorldHostUrl: remoteWorldHostUrl === null ? undefined : normalizeDedicatedWorldSocketUrl(remoteWorldHostUrl),
    };
  }

  return { worldTransport: "worker" };
}

export function normalizeDedicatedWorldSocketUrl(value: string | null | undefined): string {
  const trimmed = value?.trim();
  if (trimmed === undefined || trimmed.length === 0) {
    return DEFAULT_DEDICATED_WORLD_SOCKET_URL;
  }

  const url = new URL(ABSOLUTE_URL_SCHEME.test(trimmed) ? trimmed : `ws://${trimmed}`);
  if (url.protocol === "http:") {
    url.protocol = "ws:";
  } else if (url.protocol === "https:") {
    url.protocol = "wss:";
  }
  if (url.protocol !== "ws:" && url.protocol !== "wss:") {
    throw new Error(`Dedicated server WebSocket URL must use http, https, ws, or wss, got ${url.protocol}`);
  }
  if (url.pathname === "/" || url.pathname.length === 0) {
    url.pathname = DEFAULT_DEDICATED_WORLD_SOCKET_PATH;
  }
  url.search = "";
  url.hash = "";
  return url.href;
}

function readWorldAuthorityTransportConfig(url: URL, worldAuthority: string): BrowserWorldTransportConfig {
  if (worldAuthority === "local") {
    return { worldTransport: "worker" };
  }
  if (worldAuthority !== "dedicated") {
    throw new Error(`Unsupported worldAuthority=${worldAuthority}`);
  }

  const netTransport = url.searchParams.get("netTransport");
  if (netTransport !== null && netTransport !== "websocket") {
    throw new Error(`Dedicated worldAuthority currently supports netTransport=websocket only, got ${netTransport}`);
  }

  return {
    worldTransport: "remote",
    remoteWorldHostUrl: normalizeDedicatedWorldSocketUrl(
      url.searchParams.get("dedicatedSocketUrl")
      ?? url.searchParams.get("dedicatedHostUrl")
      ?? url.searchParams.get("worldHostUrl"),
    ),
  };
}
