import { expect, test, type Page } from "./remote-world-host-fixture";
import { type BootResult } from "../../src/renderer/main.ts";
import { getDefaultRenderDistance, getExpectedLoadedChunkCount } from "../../src/renderer/browser-render-config.ts";

const REMOTE_SMOKE_SCREENSHOT_PATH = "/tmp/mclone-browser-remote-smoke.png";
const WORKER_SMOKE_SCREENSHOT_PATH = "/tmp/mclone-browser-worker-smoke.png";
const SMOKE_VIEW_DISTANCE = 2;
const SMOKE_RENDER_DISTANCE = getDefaultRenderDistance(SMOKE_VIEW_DISTANCE);
const EXPECTED_LOADED_CHUNK_COUNT = getExpectedLoadedChunkCount(SMOKE_VIEW_DISTANCE);

function createSmokeParams(extra: Record<string, string>): URLSearchParams {
  return new URLSearchParams({
    viewDistance: SMOKE_VIEW_DISTANCE.toString(),
    renderDistance: SMOKE_RENDER_DISTANCE.toString(),
    lightingMode: "none",
    liquidSimulationMode: "none",
    fogColor: "8fb8ff",
    cameraX: "960.5",
    cameraY: "132",
    cameraZ: "-8127.5",
    cameraYaw: "225",
    cameraPitch: "60",
    ...extra,
  });
}

function createRemoteSmokeUrl(remoteWorldHostUrl: string): string {
  return `/smoke.html?${createSmokeParams({
    worldTransport: "remote",
    worldHostUrl: remoteWorldHostUrl,
  }).toString()}`;
}

function createWorkerSmokeUrl(): string {
  return `/smoke.html?${createSmokeParams({
    worldTransport: "worker",
    worldStorageMode: "none",
  }).toString()}`;
}

async function bootPage(page: Page, url: string): Promise<BootResult> {
  await page.goto(url, { waitUntil: "load" });
  await page.waitForFunction(() => typeof window.__mcloneReady !== "undefined");
  return (await page.evaluate(() => window.__mcloneReady)) as BootResult;
}

function expectRenderedSmokeResult(
  result: BootResult,
  expectedTransport: "worker" | "remote",
): asserts result is Extract<BootResult, { ok: true }> {
  expect(result.ok, JSON.stringify(result)).toBe(true);
  if (!result.ok) {
    return;
  }

  expect(result.worldTransport).toBe(expectedTransport);
  expect(result.meshTransport).toBe("worker");
  expect(result.adapterInfo.length).toBeGreaterThan(0);
  expect(["bgra8unorm", "rgba8unorm"]).toContain(result.format);
  expect(result.loadedChunkCount).toBe(EXPECTED_LOADED_CHUNK_COUNT);
  expect(result.expectedLoadedChunkCount).toBe(EXPECTED_LOADED_CHUNK_COUNT);
  expect(result.viewDistance).toBe(SMOKE_VIEW_DISTANCE);
  expect(result.renderDistance).toBe(SMOKE_RENDER_DISTANCE);
  expect(result.lightingMode).toBe("none");
  expect(result.liquidSimulationMode).toBe("none");
  expect(result.solidDrawCount).toBeGreaterThan(0);
  expect(result.renderWorldCounters.ingestBatchCount).toBeGreaterThan(0);
  expect(result.renderWorldCounters.meshBuildRequestCount).toBeGreaterThan(0);
  expect(result.renderWorldCounters.meshCompletionCount).toBeGreaterThan(0);
  expect(result.renderWorldCounters.mainThreadGpuUploadCount).toBeGreaterThan(0);
  expect(result.renderWorldCounters.meshNotReadyResponseCount).toBeGreaterThanOrEqual(0);
  expect(result.renderQueueStats.renderedChunkCount).toBeGreaterThan(0);
  expect(result.renderQueueStats.pendingVisibleChunkCompileCount).toBe(0);
  expect(result.renderQueueStats.queuedChunkBuildCount).toBe(0);
  expect(result.renderQueueStats.activeChunkBuildCount).toBe(0);
}

function expectAuthoritativePlayerLoop(result: Extract<BootResult, { ok: true }>): void {
  expect(result.sessionId).toBeDefined();
  expect(result.playerId).toBe(result.sessionId);
  expect(result.sessionRevision).toBeGreaterThan(0);
  expect(result.playerInputSequence).toBe(1);
  expect(result.playerStateRevision).toBeGreaterThan(0);
  expect(result.playerTick).toBeGreaterThan(0);
  expect(result.playerPosition?.length).toBe(3);
}

test.setTimeout(30_000);

test("WebGPU boot succeeds against the remote Node host with two browser clients", async ({ browser, remoteWorldHostUrl }) => {
  const context = await browser.newContext();
  const firstPage = await context.newPage();
  const secondPage = await context.newPage();
  const pageErrors: string[] = [];
  firstPage.on("pageerror", (err) => pageErrors.push(`first: ${String(err)}`));
  secondPage.on("pageerror", (err) => pageErrors.push(`second: ${String(err)}`));

  const [firstResult, secondResult] = await Promise.all([
    bootPage(firstPage, createRemoteSmokeUrl(remoteWorldHostUrl)),
    bootPage(secondPage, createRemoteSmokeUrl(remoteWorldHostUrl)),
  ]);
  await firstPage.locator("#renderer").screenshot({ path: REMOTE_SMOKE_SCREENSHOT_PATH });

  expectRenderedSmokeResult(firstResult, "remote");
  expectRenderedSmokeResult(secondResult, "remote");

  expect(pageErrors, pageErrors.join("\n")).toEqual([]);
  expect(firstResult.saveId).toBe(secondResult.saveId);
  expect(firstResult.sessionId).not.toBe(secondResult.sessionId);
  expectAuthoritativePlayerLoop(firstResult);
  expectAuthoritativePlayerLoop(secondResult);
});

test("WebGPU boot succeeds against the worker integrated server", async ({ page }) => {
  const pageErrors: string[] = [];
  page.on("pageerror", (err) => pageErrors.push(String(err)));

  const result = await bootPage(page, createWorkerSmokeUrl());
  await page.locator("#renderer").screenshot({ path: WORKER_SMOKE_SCREENSHOT_PATH });

  expectRenderedSmokeResult(result, "worker");
  expectAuthoritativePlayerLoop(result);
  expect(pageErrors, pageErrors.join("\n")).toEqual([]);
});
