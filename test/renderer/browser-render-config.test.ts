import { describe, expect, test } from "vitest";
import {
  getDefaultRenderDistance,
  getExpectedLoadedChunkCount,
  readBrowserRenderConfig,
} from "../../src/renderer/browser-render-config";

describe("browser render config", () => {
  test("reads explicit view distance, render distance, and fog color overrides", () => {
    const config = readBrowserRenderConfig(new URL("http://127.0.0.1/?viewDistance=7&renderDistance=160&fogColor=8fb8ff&clearColorScale=0.5"));

    expect(config.viewDistance).toBe(7);
    expect(config.renderDistance).toBe(160);
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
});
