import { test, expect } from "@playwright/test";
import { type BootResult } from "../../src/renderer/main.ts";

const SMOKE_SCREENSHOT_PATH = "/tmp/mclone-browser-smoke.png";

test("WebGPU boot succeeds on system Chrome", async ({ page }) => {
  const pageErrors: string[] = [];
  page.on("pageerror", (err) => pageErrors.push(String(err)));

  await page.goto("/", { waitUntil: "networkidle" });
  await page.waitForFunction(() => typeof window.__mcloneReady !== "undefined");

  const result = (await page.evaluate(() => window.__mcloneReady)) as BootResult;
  await page.locator("#renderer").screenshot({ path: SMOKE_SCREENSHOT_PATH });

  expect(pageErrors, pageErrors.join("\n")).toEqual([]);
  expect(result.ok, JSON.stringify(result)).toBe(true);
  if (result.ok) {
    expect(result.adapterInfo.length).toBeGreaterThan(0);
    expect(["bgra8unorm", "rgba8unorm"]).toContain(result.format);
    expect(result.loadedChunkCount).toBeGreaterThan(0);
    expect(result.solidDrawCount).toBeGreaterThan(0);
    expect(result.cutoutDrawCount).toBeGreaterThan(0);
    expect(result.translucentDrawCount).toBeGreaterThan(0);
    expect(result.terrainPixel).not.toEqual(result.clearPixel);
    const pixelDelta = result.terrainPixel.reduce(
      (sum, channel, index) => sum + Math.abs(channel - result.clearPixel[index]!),
      0,
    );
    expect(pixelDelta).toBeGreaterThan(20);
  }
});
