import { expect, test, type Page } from "./remote-world-host-fixture";

const DEBUG_SCREENSHOT_PATH = "/tmp/mclone-debug-free-cam.png";
const DEBUG_TALL_SCREENSHOT_PATH = "/tmp/mclone-debug-free-cam-tall.png";
const EXPECTED_LOADED_CHUNK_COUNT = 225;

test.setTimeout(60_000);

interface RenderWorldPerformanceCounters {
  readonly ingestBatchCount: number;
  readonly meshBuildRequestCount: number;
  readonly meshNotReadyResponseCount: number;
  readonly meshCompletionCount: number;
  readonly mainThreadGpuUploadCount: number;
}

interface RenderSceneQueueStats {
  readonly renderedChunkCount: number;
  readonly pendingVisibleChunkCompileCount: number;
  readonly queuedChunkBuildCount: number;
  readonly activeChunkBuildCount: number;
}

interface DebugRuntimeState {
  readonly ready: boolean;
  readonly worldTransport: "worker" | "remote";
  readonly playerTick?: number;
  readonly playerPosition?: readonly [number, number, number];
  readonly playerChunkX?: number;
  readonly playerChunkZ?: number;
  readonly chunkViewCenterX?: number;
  readonly chunkViewCenterZ?: number;
  readonly loadedChunkCount: number;
  readonly expectedLoadedChunkCount?: number;
  readonly viewDistance?: number;
  readonly renderDistance?: number;
  readonly frameCount: number;
  readonly renderWorldCounters?: RenderWorldPerformanceCounters;
  readonly renderQueueStats?: RenderSceneQueueStats;
  readonly error?: string;
}

interface CanvasMetrics {
  readonly width: number;
  readonly height: number;
  readonly clientWidth: number;
  readonly clientHeight: number;
  readonly devicePixelRatio: number;
}

function createDebugUrl(remoteWorldHostUrl: string): string {
  return `/debug.html?${new URLSearchParams({
    worldTransport: "remote",
    worldHostUrl: remoteWorldHostUrl,
    preserveInitialCamera: "1",
    cameraX: "965.5",
    cameraY: "168",
    cameraZ: "3189.5",
    cameraYaw: "225",
    cameraPitch: "55",
    viewDistance: "6",
    renderDistance: "192",
    fogColor: "8fb8ff",
  }).toString()}`;
}

async function readDebugState(page: Page): Promise<DebugRuntimeState> {
  return await page.evaluate(() => window.__mcloneDebug!.state as DebugRuntimeState);
}

async function readCanvasMetrics(page: Page): Promise<CanvasMetrics> {
  return await page.evaluate(() => {
    const canvas = document.querySelector<HTMLCanvasElement>("#renderer");
    if (!canvas) {
      throw new Error("canvas #renderer not found");
    }

    return {
      width: canvas.width,
      height: canvas.height,
      clientWidth: canvas.clientWidth,
      clientHeight: canvas.clientHeight,
      devicePixelRatio: window.devicePixelRatio,
    };
  });
}

async function waitForDebugReady(page: Page): Promise<void> {
  await page.waitForFunction(() => typeof window.__mcloneDebug !== "undefined", undefined, { timeout: 20_000 });
  await page.waitForFunction(
    () => window.__mcloneDebug!.state.ready === true || window.__mcloneDebug!.state.error !== undefined,
    undefined,
    { timeout: 20_000 },
  );
}

function expectRenderQueueSettled(state: DebugRuntimeState): void {
  expect(state.renderQueueStats).toBeDefined();
  expect(state.renderQueueStats!.renderedChunkCount).toBeGreaterThan(0);
  expect(state.renderQueueStats!.pendingVisibleChunkCompileCount).toBe(0);
  expect(state.renderQueueStats!.queuedChunkBuildCount).toBe(0);
  expect(state.renderQueueStats!.activeChunkBuildCount).toBe(0);
}

test("debug free-cam follows authoritative player_state and moves chunk interest with remote input", async ({ page, remoteWorldHostUrl }) => {
  await page.goto(createDebugUrl(remoteWorldHostUrl), { waitUntil: "load" });
  await waitForDebugReady(page);
  await page.waitForFunction(
    (expectedLoadedChunkCount) => {
      const state = window.__mcloneDebug?.state;
      return (state?.loadedChunkCount ?? 0) === expectedLoadedChunkCount && (state?.frameCount ?? 0) >= 10;
    },
    EXPECTED_LOADED_CHUNK_COUNT,
    { timeout: 20_000 },
  );

  const initialState = await readDebugState(page);
  expect(initialState.error).toBeUndefined();
  expect(initialState.worldTransport).toBe("remote");
  expect(initialState.loadedChunkCount).toBe(EXPECTED_LOADED_CHUNK_COUNT);
  expect(initialState.expectedLoadedChunkCount).toBe(EXPECTED_LOADED_CHUNK_COUNT);
  expect(initialState.viewDistance).toBe(6);
  expect(initialState.renderDistance).toBe(192);
  expect(initialState.playerChunkZ).toBeDefined();
  expect(initialState.chunkViewCenterZ).toBeDefined();
  expect(initialState.renderWorldCounters).toBeDefined();
  expect(initialState.renderWorldCounters!.ingestBatchCount).toBeGreaterThan(0);
  expect(initialState.renderWorldCounters!.meshBuildRequestCount).toBeGreaterThan(0);
  expect(initialState.renderWorldCounters!.meshCompletionCount).toBeGreaterThan(0);
  expect(initialState.renderWorldCounters!.mainThreadGpuUploadCount).toBeGreaterThan(0);
  expectRenderQueueSettled(initialState);
  await page.locator("#renderer").screenshot({ path: DEBUG_SCREENSHOT_PATH });

  await page.evaluate(() => {
    window.__mcloneDebug!.setInjectedInput({
      locked: true,
      heldKeys: ["KeyW"],
    });
  });

  await page.waitForFunction(
    ({ initialChunkX, initialChunkZ, initialTick }) => {
      const state = window.__mcloneDebug?.state;
      return state?.playerChunkZ !== undefined
        && state.playerChunkX !== undefined
        && state.chunkViewCenterZ !== undefined
        && state.chunkViewCenterX !== undefined
        && (state.playerChunkX !== initialChunkX || state.playerChunkZ !== initialChunkZ)
        && state.playerChunkX === state.chunkViewCenterX
        && state.playerChunkZ === state.chunkViewCenterZ
        && state.playerTick !== undefined
        && state.playerTick > initialTick;
    },
    {
      initialChunkX: initialState.playerChunkX!,
      initialChunkZ: initialState.playerChunkZ!,
      initialTick: initialState.playerTick ?? 0,
    },
    { timeout: 10_000 },
  );

  await page.evaluate(() => {
    window.__mcloneDebug!.setInjectedInput(null);
  });
  await page.waitForFunction(
    (expectedLoadedChunkCount) => {
      const state = window.__mcloneDebug?.state;
      return (state?.loadedChunkCount ?? 0) === expectedLoadedChunkCount;
    },
    EXPECTED_LOADED_CHUNK_COUNT,
    { timeout: 20_000 },
  );

  const movedState = await readDebugState(page);
  expect(movedState.playerPosition).not.toEqual(initialState.playerPosition);
  expect(movedState.playerChunkX).toBe(movedState.chunkViewCenterX);
  expect(movedState.playerChunkZ).toBe(movedState.chunkViewCenterZ);
  expect(movedState.loadedChunkCount).toBe(EXPECTED_LOADED_CHUNK_COUNT);
  expect(movedState.renderWorldCounters).toBeDefined();
  expect(movedState.renderWorldCounters!.ingestBatchCount).toBeGreaterThan(initialState.renderWorldCounters!.ingestBatchCount);
  expect(movedState.renderWorldCounters!.meshBuildRequestCount).toBeGreaterThanOrEqual(initialState.renderWorldCounters!.meshBuildRequestCount);
  expect(movedState.renderWorldCounters!.meshCompletionCount).toBeGreaterThanOrEqual(initialState.renderWorldCounters!.meshCompletionCount);
  expect(movedState.renderWorldCounters!.mainThreadGpuUploadCount).toBeGreaterThanOrEqual(initialState.renderWorldCounters!.mainThreadGpuUploadCount);
});

test("debug free-cam keeps the backing buffer aligned with a tall viewport after resize", async ({ page, remoteWorldHostUrl }) => {
  await page.setViewportSize({ width: 1280, height: 720 });
  await page.goto(createDebugUrl(remoteWorldHostUrl), { waitUntil: "load" });
  await waitForDebugReady(page);
  await page.waitForFunction(() => (window.__mcloneDebug?.state.frameCount ?? 0) >= 10, undefined, { timeout: 20_000 });

  await page.setViewportSize({ width: 430, height: 932 });
  await page.waitForFunction(() => {
    const canvas = document.querySelector<HTMLCanvasElement>("#renderer");
    if (!canvas) {
      return false;
    }

    return canvas.clientHeight > canvas.clientWidth
      && Math.abs(canvas.width - Math.round(canvas.clientWidth * window.devicePixelRatio)) <= 1
      && Math.abs(canvas.height - Math.round(canvas.clientHeight * window.devicePixelRatio)) <= 1
      && (window.__mcloneDebug?.state.frameCount ?? 0) >= 20;
  }, undefined, { timeout: 20_000 });

  const state = await readDebugState(page);
  const metrics = await readCanvasMetrics(page);
  await page.locator("#renderer").screenshot({ path: DEBUG_TALL_SCREENSHOT_PATH });

  expect(state.error).toBeUndefined();
  expectRenderQueueSettled(state);
  expect(metrics.clientHeight).toBeGreaterThan(metrics.clientWidth);
  expect(Math.abs(metrics.width - Math.round(metrics.clientWidth * metrics.devicePixelRatio))).toBeLessThanOrEqual(1);
  expect(Math.abs(metrics.height - Math.round(metrics.clientHeight * metrics.devicePixelRatio))).toBeLessThanOrEqual(1);
  expect(Math.abs((metrics.width / metrics.height) - (metrics.clientWidth / metrics.clientHeight))).toBeLessThan(0.01);
});
