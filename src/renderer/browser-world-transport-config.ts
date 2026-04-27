export type BrowserWorldTransport = "worker" | "remote";
export type BrowserWorldAuthority = "local" | "dedicated";

export interface BrowserWorldTransportConfig {
  readonly worldTransport: BrowserWorldTransport;
  readonly remoteWorldHostUrl?: string;
}

export interface BrowserWorldTransportSettings {
  readonly worldAuthority: BrowserWorldAuthority;
  readonly dedicatedSocketUrl: string;
}

export const DEFAULT_DEDICATED_WORLD_SOCKET_URL = "ws://127.0.0.1:4173/api/world/socket";
export const BROWSER_WORLD_TRANSPORT_SETTINGS_STORAGE_KEY = "mclone.world.transport.v1";

const DEFAULT_DEDICATED_WORLD_SOCKET_PATH = "/api/world/socket";
const ABSOLUTE_URL_SCHEME = /^[A-Za-z][A-Za-z0-9+.-]*:\/\//;
const DEFAULT_BROWSER_WORLD_TRANSPORT_SETTINGS: BrowserWorldTransportSettings = {
  worldAuthority: "local",
  dedicatedSocketUrl: DEFAULT_DEDICATED_WORLD_SOCKET_URL,
};

export function readBrowserWorldTransportConfig(
  url: URL,
  storage?: Pick<Storage, "getItem">,
): BrowserWorldTransportConfig {
  return resolveBrowserWorldTransportConfig(readBrowserWorldTransportSettings(url, storage));
}

export function resolveBrowserWorldTransportConfig(settings: BrowserWorldTransportSettings): BrowserWorldTransportConfig {
  if (settings.worldAuthority === "dedicated") {
    return {
      worldTransport: "remote",
      remoteWorldHostUrl: normalizeDedicatedWorldSocketUrl(settings.dedicatedSocketUrl),
    };
  }

  return { worldTransport: "worker" };
}

export function readBrowserWorldTransportSettings(
  url: URL,
  storage?: Pick<Storage, "getItem">,
): BrowserWorldTransportSettings {
  const stored = readStoredBrowserWorldTransportSettings(storage);
  let worldAuthority = stored.worldAuthority ?? DEFAULT_BROWSER_WORLD_TRANSPORT_SETTINGS.worldAuthority;
  let dedicatedSocketUrl = stored.dedicatedSocketUrl ?? DEFAULT_BROWSER_WORLD_TRANSPORT_SETTINGS.dedicatedSocketUrl;

  const worldAuthorityParam = url.searchParams.get("worldAuthority");
  const worldTransport = url.searchParams.get("worldTransport");
  const serverParam = url.searchParams.get("server");
  if (worldAuthorityParam !== null) {
    if (worldAuthorityParam !== "local" && worldAuthorityParam !== "dedicated") {
      throw new Error(`Unsupported worldAuthority=${worldAuthorityParam}`);
    }
    worldAuthority = worldAuthorityParam;
  } else if (serverParam !== null) {
    worldAuthority = "dedicated";
  } else if (worldTransport === "remote") {
    worldAuthority = "dedicated";
  } else if (worldTransport === "worker") {
    worldAuthority = "local";
  }

  const dedicatedSocketUrlParam = url.searchParams.get("dedicatedSocketUrl")
    ?? url.searchParams.get("dedicatedHostUrl")
    ?? url.searchParams.get("worldHostUrl")
    ?? serverParam;
  if (dedicatedSocketUrlParam !== null) {
    dedicatedSocketUrl = sanitizeDedicatedWorldSocketInput(dedicatedSocketUrlParam);
  }

  const netTransport = url.searchParams.get("netTransport");
  if (worldAuthority === "dedicated" && netTransport !== null && netTransport !== "websocket") {
    throw new Error(`Dedicated worldAuthority currently supports netTransport=websocket only, got ${netTransport}`);
  }

  return {
    worldAuthority,
    dedicatedSocketUrl,
  };
}

export function hasDedicatedServerJoinParam(url: URL): boolean {
  return url.searchParams.has("server");
}

export function readStoredBrowserWorldTransportSettings(
  storage: Pick<Storage, "getItem"> | undefined,
): Partial<BrowserWorldTransportSettings> {
  if (storage === undefined) {
    return {};
  }

  try {
    const raw = storage.getItem(BROWSER_WORLD_TRANSPORT_SETTINGS_STORAGE_KEY);
    if (raw === null) {
      return {};
    }

    const parsed = JSON.parse(raw) as unknown;
    if (!isRecord(parsed)) {
      return {};
    }

    return {
      worldAuthority: parseWorldAuthority(parsed.worldAuthority),
      dedicatedSocketUrl: typeof parsed.dedicatedSocketUrl === "string"
        ? sanitizeDedicatedWorldSocketInput(parsed.dedicatedSocketUrl)
        : undefined,
    };
  } catch {
    return {};
  }
}

export function writeStoredBrowserWorldTransportSettings(
  storage: Pick<Storage, "setItem">,
  settings: BrowserWorldTransportSettings,
): void {
  storage.setItem(BROWSER_WORLD_TRANSPORT_SETTINGS_STORAGE_KEY, JSON.stringify({
    worldAuthority: settings.worldAuthority,
    dedicatedSocketUrl: sanitizeDedicatedWorldSocketInput(settings.dedicatedSocketUrl),
  }));
}

export function normalizeDedicatedWorldSocketUrl(value: string | null | undefined): string {
  const trimmed = sanitizeDedicatedWorldSocketInput(value);
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

function sanitizeDedicatedWorldSocketInput(value: string | null | undefined): string {
  const trimmed = value?.trim();
  if (trimmed === undefined || trimmed.length === 0) {
    return DEFAULT_DEDICATED_WORLD_SOCKET_URL;
  }
  return trimmed;
}

function parseWorldAuthority(value: unknown): BrowserWorldAuthority | undefined {
  return value === "local" || value === "dedicated" ? value : undefined;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}
