import { describe, expect, test } from "vitest";
import {
  BROWSER_RENDER_CONFIG_STORAGE_KEY,
  clearStoredBrowserRenderConfig,
  getDefaultRenderDistance,
  getExpectedLoadedChunkCount,
  readBrowserRenderConfig,
  readStoredBrowserRenderConfig,
  writeStoredBrowserRenderConfig,
} from "../../src/renderer/browser-render-config";

class TestStorage {
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

describe("browser render config", () => {
  test("reads explicit view distance, render distance, and fog color overrides", () => {
    const config = readBrowserRenderConfig(new URL("http://127.0.0.1/?viewDistance=7&renderDistance=160&fogColor=8fb8ff&clearColorScale=0.5"));

    expect(config.viewDistance).toBe(7);
    expect(config.renderDistance).toBe(160);
    expect(config.lightingMode).toBe("vanilla17");
    expect(config.liquidSimulationMode).toBe("vanilla17");
    expect(config.skyColor.x).toBeCloseTo(0x8f / 255);
    expect(config.skyColor.y).toBeCloseTo(0xb8 / 255);
    expect(config.skyColor.z).toBeCloseTo(0xff / 255);
    expect(config.clearColorScale).toBe(0.5);
  });

  test("falls back for invalid overrides and derives render distance from the chunk radius", () => {
    const config = readBrowserRenderConfig(new URL("http://127.0.0.1/?viewDistance=nope&fogColor=bad"));

    expect(config.viewDistance).toBe(6);
    expect(config.renderDistance).toBe(192);
    expect(config.skyColor.x).toBeCloseTo(0x8f / 255);
    expect(config.skyColor.y).toBeCloseTo(0xb8 / 255);
    expect(config.skyColor.z).toBeCloseTo(0xff / 255);
  });

  test("derives render distance and loaded chunk targets from the configured view distance", () => {
    expect(getDefaultRenderDistance(6)).toBe(192);
    expect(getExpectedLoadedChunkCount(6)).toBe(225);
  });

  test("uses debug local storage for chunk and engine settings", () => {
    const storage = new TestStorage();
    writeStoredBrowserRenderConfig(storage, {
      viewDistance: 3,
      renderDistance: 96,
      lightingMode: "none",
      liquidSimulationMode: "none",
    });

    const config = readBrowserRenderConfig(new URL("http://127.0.0.1/"), storage);

    expect(config.viewDistance).toBe(3);
    expect(config.renderDistance).toBe(96);
    expect(config.lightingMode).toBe("none");
    expect(config.liquidSimulationMode).toBe("none");
    expect(readStoredBrowserRenderConfig(storage)).toEqual({
      viewDistance: 3,
      renderDistance: 96,
      lightingMode: "none",
      liquidSimulationMode: "none",
    });
  });

  test("lets query params override debug local storage", () => {
    const storage = new TestStorage();
    storage.setItem(BROWSER_RENDER_CONFIG_STORAGE_KEY, JSON.stringify({
      viewDistance: 2,
      renderDistance: 64,
      lightingMode: "none",
      liquidSimulationMode: "none",
    }));

    const config = readBrowserRenderConfig(
      new URL("http://127.0.0.1/?viewDistance=5&renderDistance=128&lightingMode=vanilla17&disableWaterSim=0"),
      storage,
    );

    expect(config.viewDistance).toBe(5);
    expect(config.renderDistance).toBe(128);
    expect(config.lightingMode).toBe("vanilla17");
    expect(config.liquidSimulationMode).toBe("vanilla17");

    clearStoredBrowserRenderConfig(storage);
    expect(readStoredBrowserRenderConfig(storage)).toEqual({});
  });
});
