import { expect, test, type Page } from "./remote-world-host-fixture";
import { type BootResult, type GpuTitleBootResult } from "../../src/renderer/main.ts";
import {
  createGeneratedWorldSmokeSearchParams,
  GENERATED_WORLD_SMOKE_SCENARIO,
  GENERATED_WORLD_TICK_CADENCE_SCENARIO,
  GENERATED_WORLD_TRANSITION_SCENARIO,
  type GeneratedWorldSmokeScenario,
  validateGeneratedWorldSmokeResult,
} from "../../src/renderer/generated-world-smoke-scenario.ts";

const REMOTE_SMOKE_SCREENSHOT_PATH = "/tmp/mclone-browser-remote-smoke.png";
const DEDICATED_QUERY_AUTO_START_SCREENSHOT_PATH = "/tmp/mclone-browser-dedicated-query-auto-start-smoke.png";
const WORKER_SMOKE_SCREENSHOT_PATH = "/tmp/mclone-browser-worker-smoke.png";
const WORKER_TRANSITION_SMOKE_SCREENSHOT_PATH = "/tmp/mclone-browser-worker-transition-smoke.png";
const WORKER_TICK_CADENCE_SMOKE_SCREENSHOT_PATH = "/tmp/mclone-browser-worker-tick-cadence-smoke.png";

interface GpuGuiState {
  readonly ready: boolean;
  readonly mode: "title" | "loading" | "world" | "paused" | "error";
  readonly screenTitle: string;
  readonly lastAction?: string;
  readonly worldReady?: boolean;
  readonly pauseScreenActive?: boolean;
  readonly frameCount: number;
  readonly inputEventCount: number;
  readonly worldTransport?: "worker" | "remote";
  readonly cameraPosition?: readonly [number, number, number];
  readonly cameraYaw?: number;
  readonly cameraPitch?: number;
  readonly playerTick?: number;
  readonly error?: string;
  readonly width: number;
  readonly height: number;
}

function createSmokeParams(
  extra: Record<string, string>,
  scenario: GeneratedWorldSmokeScenario = GENERATED_WORLD_SMOKE_SCENARIO,
): URLSearchParams {
  return createGeneratedWorldSmokeSearchParams(extra, scenario);
}

function createRemoteSmokeUrl(remoteWorldHostUrl: string): string {
  return `/smoke.html?${createSmokeParams({
    worldTransport: "remote",
    worldHostUrl: remoteWorldHostUrl,
  }).toString()}`;
}

function createDedicatedQueryAutoStartUrl(remoteWorldHostUrl: string): string {
  const dedicatedSocketUrl = new URL(remoteWorldHostUrl).host;
  return `/smoke.html?${createSmokeParams({
    worldAuthority: "dedicated",
    dedicatedSocketUrl,
    netTransport: "websocket",
    startWorld: "1",
    gpuTitle: "1",
  }).toString()}`;
}

function createWorkerSmokeUrl(): string {
  return `/smoke.html?${createSmokeParams({
    worldTransport: "worker",
    worldStorageMode: "none",
  }).toString()}`;
}

function createWorkerTransitionSmokeUrl(): string {
  return `/smoke.html?${createSmokeParams({
    worldTransport: "worker",
    worldStorageMode: "none",
  }, GENERATED_WORLD_TRANSITION_SCENARIO).toString()}`;
}

function createWorkerTickCadenceSmokeUrl(): string {
  return `/smoke.html?${createSmokeParams({
    worldTransport: "worker",
    worldStorageMode: "none",
  }, GENERATED_WORLD_TICK_CADENCE_SCENARIO).toString()}`;
}

async function bootPage(page: Page, url: string): Promise<BootResult> {
  await page.goto(url, { waitUntil: "load" });
  await page.waitForFunction(() => typeof window.__mcloneReady !== "undefined");
  return (await page.evaluate(() => window.__mcloneReady)) as BootResult;
}

async function readGpuGuiState(page: Page): Promise<GpuGuiState> {
  return await page.evaluate(() => window.__mcloneGui!.state as GpuGuiState);
}

function expectRenderedSmokeResult(
  result: BootResult,
  expectedTransport: "worker" | "remote",
  scenario: GeneratedWorldSmokeScenario = GENERATED_WORLD_SMOKE_SCENARIO,
): asserts result is Extract<BootResult, { ok: true }> {
  expect(result.ok, JSON.stringify(result)).toBe(true);
  if (!result.ok) {
    return;
  }

  expect(result.worldTransport).toBe(expectedTransport);
  expect(result.meshTransport).toBe("worker");
  expect(result.adapterInfo.length).toBeGreaterThan(0);
  expect(["bgra8unorm", "rgba8unorm"]).toContain(result.format);
  expect(validateGeneratedWorldSmokeResult(result, {
    expectedWorldTransport: expectedTransport,
    requirePlayerInput: true,
    requireSteps: true,
  }, scenario)).toEqual([]);
}

test.setTimeout(60_000);

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
  expect(firstResult.playerId).not.toBe(secondResult.playerId);
});

test("GPU title auto-starts a dedicated WebSocket session from query params", async ({ page, remoteWorldHostUrl }) => {
  const pageErrors: string[] = [];
  page.on("pageerror", (err) => pageErrors.push(String(err)));

  await page.goto(createDedicatedQueryAutoStartUrl(remoteWorldHostUrl), { waitUntil: "load" });
  await page.waitForFunction(() => typeof window.__mcloneReady !== "undefined");
  const titleResult = (await page.evaluate(() => window.__mcloneReady)) as GpuTitleBootResult | { readonly ok: false; readonly reason: string };
  expect(titleResult.ok, JSON.stringify(titleResult)).toBe(true);

  await page.waitForFunction(
    () => window.__mcloneGui?.state.worldReady === true || window.__mcloneGui?.state.mode === "error",
    undefined,
    { timeout: 100_000 },
  );
  await page.locator("#renderer").screenshot({ path: DEDICATED_QUERY_AUTO_START_SCREENSHOT_PATH });

  const state = await readGpuGuiState(page);
  expect(state.mode, state.error).toBe("world");
  expect(state.worldTransport).toBe("remote");
  expect(state.worldReady).toBe(true);
  expect(pageErrors, pageErrors.join("\n")).toEqual([]);
});

test("WebGPU boot succeeds against the worker integrated server", async ({ page }) => {
  const pageErrors: string[] = [];
  page.on("pageerror", (err) => pageErrors.push(String(err)));

  const result = await bootPage(page, createWorkerSmokeUrl());
  await page.locator("#renderer").screenshot({ path: WORKER_SMOKE_SCREENSHOT_PATH });

  expectRenderedSmokeResult(result, "worker");
  expect(pageErrors, pageErrors.join("\n")).toEqual([]);
});

test("WebGPU generated-world transition scenario settles against the worker integrated server", async ({ page }) => {
  const pageErrors: string[] = [];
  page.on("pageerror", (err) => pageErrors.push(String(err)));

  const result = await bootPage(page, createWorkerTransitionSmokeUrl());
  await page.locator("#renderer").screenshot({ path: WORKER_TRANSITION_SMOKE_SCREENSHOT_PATH });

  expectRenderedSmokeResult(result, "worker", GENERATED_WORLD_TRANSITION_SCENARIO);
  expect(result.steps.map((step) => step.stepName)).toEqual(["initial", "shifted"]);
  expect(result.steps[0]?.chunkCenter).not.toEqual(result.steps[1]?.chunkCenter);
  expect(pageErrors, pageErrors.join("\n")).toEqual([]);
});

test("WebGPU generated-world tick cadence scenario advances against the worker integrated server", async ({ page }) => {
  const pageErrors: string[] = [];
  page.on("pageerror", (err) => pageErrors.push(String(err)));

  const result = await bootPage(page, createWorkerTickCadenceSmokeUrl());
  await page.locator("#renderer").screenshot({ path: WORKER_TICK_CADENCE_SMOKE_SCREENSHOT_PATH });

  expectRenderedSmokeResult(result, "worker", GENERATED_WORLD_TICK_CADENCE_SCENARIO);
  expect(result.steps.map((step) => step.stepName)).toEqual([
    "tick-01-forward",
    "tick-02-diagonal",
    "tick-03-strafe",
    "tick-04-release",
  ]);
  expect(result.steps.map((step) => step.playerInputSequence)).toEqual([1, 2, 3, 4]);
  expect(result.steps.map((step) => step.frameIndex)).toEqual([0, 1, 2, 3]);
  expect(result.steps.at(-1)?.playerTick).toBeGreaterThan(result.steps[0]?.playerTick ?? 0);
  expect(result.steps.at(-1)?.playerStateRevision).toBeGreaterThan(result.steps[0]?.playerStateRevision ?? 0);
  expect(result.steps.at(-1)?.playerPosition).not.toEqual(result.steps[0]?.playerPosition);
  expect(pageErrors, pageErrors.join("\n")).toEqual([]);
});

test("GPU title-started world requires pointer lock before mouse-look but not physics stepping", async ({ page }) => {
  const pageErrors: string[] = [];
  page.on("pageerror", (err) => pageErrors.push(String(err)));

  const params = createSmokeParams({
    worldTransport: "worker",
    worldStorageMode: "none",
    gpuTitle: "1",
  });

  await page.goto(`/smoke.html?${params.toString()}`, { waitUntil: "load" });
  await page.waitForFunction(() => typeof window.__mcloneReady !== "undefined");
  const titleResult = (await page.evaluate(() => window.__mcloneReady)) as GpuTitleBootResult | { readonly ok: false; readonly reason: string };
  expect(titleResult.ok, JSON.stringify(titleResult)).toBe(true);
  await page.waitForFunction(() => window.__mcloneGui?.state.ready === true, undefined, { timeout: 20_000 });

  const box = await page.locator("#renderer").boundingBox();
  expect(box).not.toBeNull();
  const titleState = await readGpuGuiState(page);
  const startWorldCenterY = (Math.floor(titleState.height / 4) + 48 + 10) / titleState.height;
  await page.mouse.click(box!.x + (box!.width / 2), box!.y + (box!.height * startWorldCenterY));

  await page.waitForFunction(
    () => window.__mcloneGui?.state.worldReady === true || window.__mcloneGui?.state.mode === "error",
    undefined,
    { timeout: 100_000 },
  );
  await page.waitForFunction(
    () => (window.__mcloneGui?.state.frameCount ?? 0) >= 3,
    undefined,
    { timeout: 20_000 },
  );

  const unlockedState = await readGpuGuiState(page);
  expect(unlockedState.mode, unlockedState.error).toBe("world");
  expect(unlockedState.cameraYaw).toBeDefined();
  expect(unlockedState.cameraPitch).toBeDefined();
  expect(unlockedState.playerTick).toBeDefined();
  await page.mouse.move(box!.x + (box!.width / 2), box!.y + (box!.height / 2));
  await page.mouse.move(box!.x + (box!.width / 2) + 24, box!.y + (box!.height / 2) + 6, { steps: 3 });
  await page.waitForFunction(
    ({ frameCount, playerTick }) => {
      const state = window.__mcloneGui?.state;
      return state !== undefined
        && state.frameCount >= frameCount + 3
        && (state.playerTick ?? playerTick) > playerTick;
    },
    { frameCount: unlockedState.frameCount, playerTick: unlockedState.playerTick ?? -1 },
    { timeout: 20_000 },
  );
  const stillUnlockedState = await readGpuGuiState(page);
  expect(stillUnlockedState.playerTick).toBeGreaterThan(unlockedState.playerTick ?? -1);
  expect(stillUnlockedState.cameraYaw).toBe(unlockedState.cameraYaw);
  expect(stillUnlockedState.cameraPitch).toBe(unlockedState.cameraPitch);

  await page.mouse.click(box!.x + (box!.width / 2), box!.y + (box!.height / 2));
  await page.waitForFunction(
    () => document.pointerLockElement === document.querySelector("#renderer"),
    undefined,
    { timeout: 20_000 },
  );

  const lockedState = await readGpuGuiState(page);
  await page.mouse.move(box!.x + (box!.width / 2) + 48, box!.y + (box!.height / 2) + 12, { steps: 3 });
  await page.waitForFunction(
    ({ frameCount, inputEventCount, cameraYaw, cameraPitch }) => {
      const state = window.__mcloneGui?.state;
      return state !== undefined
        && state.frameCount >= frameCount + 3
        && state.inputEventCount > inputEventCount
        && (state.cameraYaw !== cameraYaw || state.cameraPitch !== cameraPitch);
    },
    {
      frameCount: lockedState.frameCount,
      inputEventCount: lockedState.inputEventCount,
      cameraYaw: lockedState.cameraYaw,
      cameraPitch: lockedState.cameraPitch,
    },
    { timeout: 20_000 },
  );

  await page.keyboard.press("Escape");
  await page.waitForFunction(
    () => document.pointerLockElement === null && window.__mcloneGui?.state.mode === "paused",
    undefined,
    { timeout: 20_000 },
  );
  const pausedState = await readGpuGuiState(page);
  expect(pausedState.pauseScreenActive).toBe(true);

  const disconnectCenterY = (Math.floor(pausedState.height / 4) + 120 - 16 + 10) / pausedState.height;
  await page.mouse.click(box!.x + (box!.width / 2), box!.y + (box!.height * disconnectCenterY));
  await page.waitForFunction(
    () => window.__mcloneGui?.state.mode === "title" && window.__mcloneGui?.state.screenTitle === "Title Screen",
    undefined,
    { timeout: 20_000 },
  );
  const returnedTitleState = await readGpuGuiState(page);
  expect(returnedTitleState.lastAction).toBe("disconnect");

  const restartStartWorldCenterY = (Math.floor(returnedTitleState.height / 4) + 48 + 10) / returnedTitleState.height;
  await page.mouse.click(box!.x + (box!.width / 2), box!.y + (box!.height * restartStartWorldCenterY));
  await page.waitForFunction(
    () => window.__mcloneGui?.state.worldReady === true || window.__mcloneGui?.state.mode === "error",
    undefined,
    { timeout: 100_000 },
  );
  const restartedWorldState = await readGpuGuiState(page);
  expect(restartedWorldState.mode, restartedWorldState.error).toBe("world");
  expect(pageErrors, pageErrors.join("\n")).toEqual([]);
});
