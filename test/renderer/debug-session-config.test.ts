import { describe, expect, it } from "vitest";
import {
  DEBUG_SESSION_CONFIG_STORAGE_KEY,
  START_LAST_WORLD_STORAGE_KEY,
  clearStoredDebugSessionConfig,
  parseSeedValue,
  readDebugSessionConfig,
  writeStartLastWorld,
  writeStoredDebugSessionConfig,
} from "../../src/renderer/debug/debug-session-config";

describe("debug session config", () => {
  it("reads query params over stored debug settings", () => {
    const storage = new MemoryStorage();
    writeStoredDebugSessionConfig(storage, {
      seed: 99n,
      movementMode: "freecam",
      preset: "flat_grass",
      showDebugInfo: true,
    });

    const config = readDebugSessionConfig(
      new URL("http://127.0.0.1/?mode=debug&seed=123&movementMode=player&preset=small_island&showDebugInfo=0"),
      storage,
    );

    expect(config).toEqual({
      seed: 123n,
      movementMode: "player",
      preset: "small_island",
      showDebugInfo: false,
    });
  });

  it("persists and clears the same keys used by /?mode=debug", () => {
    const storage = new MemoryStorage();
    const config = {
      seed: 12_345n,
      movementMode: "player" as const,
      preset: "browser_smoke" as const,
      showDebugInfo: true,
    };

    writeStoredDebugSessionConfig(storage, config);
    writeStartLastWorld(storage, config);
    expect(storage.getItem(DEBUG_SESSION_CONFIG_STORAGE_KEY)).toContain("\"preset\":\"browser_smoke\"");
    expect(storage.getItem(START_LAST_WORLD_STORAGE_KEY)).toContain("\"movementMode\":\"player\"");
    expect(storage.getItem(START_LAST_WORLD_STORAGE_KEY)).toContain("\"showDebugInfo\":true");

    clearStoredDebugSessionConfig(storage);
    expect(storage.getItem(DEBUG_SESSION_CONFIG_STORAGE_KEY)).toBeNull();
    expect(parseSeedValue("not-a-number", 7n)).toBe(7n);
  });
});

class MemoryStorage {
  private readonly values = new Map<string, string>();

  public getItem(key: string): string | null {
    return this.values.get(key) ?? null;
  }

  public setItem(key: string, value: string): void {
    this.values.set(key, value);
  }

  public removeItem(key: string): void {
    this.values.delete(key);
  }
}
