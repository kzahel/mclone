import { describe, expect, test } from "vitest";
import {
  DEFAULT_DEDICATED_WORLD_SOCKET_URL,
  BROWSER_WORLD_TRANSPORT_SETTINGS_STORAGE_KEY,
  normalizeDedicatedWorldSocketUrl,
  readBrowserWorldTransportConfig,
  readBrowserWorldTransportSettings,
  writeStoredBrowserWorldTransportSettings,
} from "../../src/renderer/browser-world-transport-config";

describe("browser world transport config", () => {
  test("defaults to the local browser worker authority", () => {
    expect(readBrowserWorldTransportConfig(url("/?mode=debug"))).toEqual({
      worldTransport: "worker",
    });
  });

  test("reads explicit local authority", () => {
    expect(readBrowserWorldTransportConfig(url("/?worldAuthority=local"))).toEqual({
      worldTransport: "worker",
    });
  });

  test("normalizes dedicated WebSocket authority shorthand", () => {
    const config = readBrowserWorldTransportConfig(
      url("/?worldAuthority=dedicated&dedicatedSocketUrl=127.0.0.1:4173&netTransport=websocket"),
    );

    expect(config).toEqual({
      worldTransport: "remote",
      remoteWorldHostUrl: "ws://127.0.0.1:4173/api/world/socket",
    });
  });

  test("treats server as a dedicated server base URL alias", () => {
    expect(readBrowserWorldTransportSettings(url("/?server=https://mclone-host.graehlarts.com"))).toEqual({
      worldAuthority: "dedicated",
      dedicatedSocketUrl: "https://mclone-host.graehlarts.com",
    });
    expect(readBrowserWorldTransportConfig(url("/?server=https://mclone-host.graehlarts.com"))).toEqual({
      worldTransport: "remote",
      remoteWorldHostUrl: "wss://mclone-host.graehlarts.com/api/world/socket",
    });
  });

  test("defaults dedicated authority to the local dedicated socket", () => {
    expect(readBrowserWorldTransportConfig(url("/?worldAuthority=dedicated"))).toEqual({
      worldTransport: "remote",
      remoteWorldHostUrl: DEFAULT_DEDICATED_WORLD_SOCKET_URL,
    });
  });

  test("keeps old remote query params working", () => {
    const config = readBrowserWorldTransportConfig(
      url("/?worldTransport=remote&worldHostUrl=http://127.0.0.1:4173"),
    );

    expect(config).toEqual({
      worldTransport: "remote",
      remoteWorldHostUrl: "ws://127.0.0.1:4173/api/world/socket",
    });
  });

  test("lets dedicated authority override old transport params during migration", () => {
    const config = readBrowserWorldTransportConfig(
      url("/?worldAuthority=dedicated&worldTransport=worker&dedicatedSocketUrl=localhost:4174"),
    );

    expect(config).toEqual({
      worldTransport: "remote",
      remoteWorldHostUrl: "ws://localhost:4174/api/world/socket",
    });
  });

  test("normalizes full WebSocket URLs without query or hash", () => {
    expect(normalizeDedicatedWorldSocketUrl("wss://example.test/world/socket?debug=1#tail")).toBe(
      "wss://example.test/world/socket",
    );
  });

  test("rejects unsupported dedicated transport carriers", () => {
    expect(() => readBrowserWorldTransportConfig(url("/?worldAuthority=dedicated&netTransport=webrtc"))).toThrow(
      "netTransport=websocket",
    );
  });

  test("reads persisted authority when query params are absent", () => {
    const storage = new MemoryStorage();
    writeStoredBrowserWorldTransportSettings(storage, {
      worldAuthority: "dedicated",
      dedicatedSocketUrl: "localhost:4179",
    });

    expect(readBrowserWorldTransportSettings(url("/?mode=debug"), storage)).toEqual({
      worldAuthority: "dedicated",
      dedicatedSocketUrl: "localhost:4179",
    });
    expect(readBrowserWorldTransportConfig(url("/?mode=debug"), storage)).toEqual({
      worldTransport: "remote",
      remoteWorldHostUrl: "ws://localhost:4179/api/world/socket",
    });
  });

  test("lets query params override persisted authority and socket independently", () => {
    const storage = new MemoryStorage();
    storage.setItem(BROWSER_WORLD_TRANSPORT_SETTINGS_STORAGE_KEY, JSON.stringify({
      worldAuthority: "dedicated",
      dedicatedSocketUrl: "localhost:4179",
    }));

    expect(readBrowserWorldTransportSettings(url("/?worldAuthority=local"), storage)).toEqual({
      worldAuthority: "local",
      dedicatedSocketUrl: "localhost:4179",
    });
    expect(readBrowserWorldTransportSettings(url("/?worldAuthority=dedicated&dedicatedSocketUrl=localhost:4180"), storage)).toEqual({
      worldAuthority: "dedicated",
      dedicatedSocketUrl: "localhost:4180",
    });
  });
});

function url(path: string): URL {
  return new URL(path, "http://127.0.0.1/");
}

class MemoryStorage {
  private readonly values = new Map<string, string>();

  public getItem(key: string): string | null {
    return this.values.get(key) ?? null;
  }

  public setItem(key: string, value: string): void {
    this.values.set(key, value);
  }
}
