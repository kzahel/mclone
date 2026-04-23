import { test, expect, type Page } from "@playwright/test";
import { type BootResult } from "../../src/renderer/main.ts";

const SMOKE_SCREENSHOT_PATH = "/tmp/mclone-browser-smoke.png";
const REMOTE_WORLD_HOST_URL = "http://127.0.0.1:4173";
const EXPECTED_LOADED_CHUNK_COUNT = 225;

function createRemoteSmokeUrl(): string {
  return `/?${new URLSearchParams({
    worldTransport: "remote",
    worldHostUrl: REMOTE_WORLD_HOST_URL,
    viewDistance: "6",
    renderDistance: "192",
    fogColor: "8fb8ff",
    cameraX: "960.5",
    cameraY: "132",
    cameraZ: "-8127.5",
    cameraYaw: "225",
    cameraPitch: "60",
  }).toString()}`;
}

async function bootPage(page: Page): Promise<BootResult> {
  await page.goto(createRemoteSmokeUrl(), { waitUntil: "networkidle" });
  await page.waitForFunction(() => typeof window.__mcloneReady !== "undefined");
  return (await page.evaluate(() => window.__mcloneReady)) as BootResult;
}

test.setTimeout(90_000);

test("WebGPU boot succeeds against the remote Node host with two browser clients", async ({ browser }) => {
  const context = await browser.newContext();
  const firstPage = await context.newPage();
  const secondPage = await context.newPage();
  const pageErrors: string[] = [];
  firstPage.on("pageerror", (err) => pageErrors.push(`first: ${String(err)}`));
  secondPage.on("pageerror", (err) => pageErrors.push(`second: ${String(err)}`));

  const [firstResult, secondResult] = await Promise.all([
    bootPage(firstPage),
    bootPage(secondPage),
  ]);
  await firstPage.locator("#renderer").screenshot({ path: SMOKE_SCREENSHOT_PATH });

  for (const result of [firstResult, secondResult]) {
    expect(result.ok, JSON.stringify(result)).toBe(true);
    if (result.ok) {
      expect(result.worldTransport).toBe("remote");
      expect(result.meshTransport).toBe("worker");
      expect(result.adapterInfo.length).toBeGreaterThan(0);
      expect(["bgra8unorm", "rgba8unorm"]).toContain(result.format);
      expect(result.loadedChunkCount).toBe(EXPECTED_LOADED_CHUNK_COUNT);
      expect(result.expectedLoadedChunkCount).toBe(EXPECTED_LOADED_CHUNK_COUNT);
      expect(result.viewDistance).toBe(6);
      expect(result.renderDistance).toBe(192);
      expect(result.solidDrawCount).toBeGreaterThan(0);
    }
  }

  expect(pageErrors, pageErrors.join("\n")).toEqual([]);
  if (firstResult.ok && secondResult.ok) {
    expect(firstResult.saveId).toBe(secondResult.saveId);
    expect(firstResult.sessionId).toBeDefined();
    expect(secondResult.sessionId).toBeDefined();
    expect(firstResult.sessionId).not.toBe(secondResult.sessionId);
    expect(firstResult.playerId).toBe(firstResult.sessionId);
    expect(secondResult.playerId).toBe(secondResult.sessionId);
    expect(firstResult.sessionRevision).toBeGreaterThan(0);
    expect(secondResult.sessionRevision).toBeGreaterThan(0);
    expect(firstResult.playerInputSequence).toBe(1);
    expect(secondResult.playerInputSequence).toBe(1);
    expect(firstResult.playerStateRevision).toBeGreaterThan(0);
    expect(secondResult.playerStateRevision).toBeGreaterThan(0);
    expect(firstResult.playerTick).toBeGreaterThan(0);
    expect(secondResult.playerTick).toBeGreaterThan(0);
    expect(firstResult.playerPosition?.length).toBe(3);
    expect(secondResult.playerPosition?.length).toBe(3);
  }
});
