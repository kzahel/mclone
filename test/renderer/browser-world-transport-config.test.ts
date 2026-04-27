import { describe, expect, test } from "vitest";
import {
  DEFAULT_DEDICATED_WORLD_SOCKET_URL,
  normalizeDedicatedWorldSocketUrl,
  readBrowserWorldTransportConfig,
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
});

function url(path: string): URL {
  return new URL(path, "http://127.0.0.1/");
}
