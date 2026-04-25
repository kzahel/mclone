import { expect, test, type Page } from "@playwright/test";
import { FAST_VISUAL_PROBE_PARAMS, FAST_VISUAL_PROBE_TIMEOUTS } from "./fast-visual-probe-config";

const CULL_VIEW_DISTANCE = "6";
const CULL_RENDER_DISTANCE = "192";
const CULL_TEST_TIMEOUT_MS = 45_000;
const CULL_SETTLE_TIMEOUT_MS = 30_000;
const CULL_YAW_145_SCREENSHOT_PATH = "/tmp/mclone-cull-smart-yaw145.png";
const CULL_YAW_144_SCREENSHOT_PATH = "/tmp/mclone-cull-smart-yaw144.png";

test.setTimeout(CULL_TEST_TIMEOUT_MS);

interface DebugRuntimeState {
  readonly ready: boolean;
  readonly frameCount: number;
  readonly loadedChunkCount: number;
  readonly renderQueueStats?: {
    readonly renderedChunkCount: number;
    readonly pendingVisibleChunkCompileCount: number;
    readonly queuedChunkBuildCount: number;
    readonly activeChunkBuildCount: number;
  };
  readonly error?: string;
}

function createCullUrl(yaw: string, y: string, clearWorldStorage: boolean): string {
  return `/debug.html?${new URLSearchParams({
    ...FAST_VISUAL_PROBE_PARAMS,
    worldTransport: "worker",
    preserveInitialCamera: "1",
    viewDistance: CULL_VIEW_DISTANCE,
    renderDistance: CULL_RENDER_DISTANCE,
    cameraX: "81.6",
    cameraY: y,
    cameraZ: "22",
    cameraYaw: yaw,
    cameraPitch: "21",
    ...(clearWorldStorage ? { clearWorldStorage: "1" } : {}),
  }).toString()}`;
}

async function waitForSettledFrame(page: Page): Promise<DebugRuntimeState> {
  await page.waitForFunction(() => typeof window.__mcloneDebug !== "undefined", undefined, {
    timeout: FAST_VISUAL_PROBE_TIMEOUTS.ready,
  });
  await page.waitForFunction(
    () => window.__mcloneDebug!.state.ready === true || window.__mcloneDebug!.state.error !== undefined,
    undefined,
    { timeout: CULL_SETTLE_TIMEOUT_MS },
  );
  await page.waitForFunction(
    () => (window.__mcloneDebug?.state.frameCount ?? 0) >= 10,
    undefined,
    { timeout: FAST_VISUAL_PROBE_TIMEOUTS.ready },
  );
  await page.waitForFunction(
    () => {
      const stats = window.__mcloneDebug?.state.renderQueueStats;
      return (
        stats !== undefined &&
        stats.pendingVisibleChunkCompileCount === 0 &&
        stats.queuedChunkBuildCount === 0 &&
        stats.activeChunkBuildCount === 0
      );
    },
    undefined,
    { timeout: CULL_SETTLE_TIMEOUT_MS },
  );
  return await page.evaluate(() => window.__mcloneDebug!.state as DebugRuntimeState);
}

async function capture(page: Page, yaw: string, y: string, clearWorldStorage: boolean, path: string): Promise<DebugRuntimeState> {
  await page.goto(createCullUrl(yaw, y, clearWorldStorage), { waitUntil: "load" });
  const state = await waitForSettledFrame(page);
  await page.locator("#renderer").screenshot({ path });
  return state;
}

test("captures settled culling frames around the reported yaw pair", async ({ page }) => {
  await page.setViewportSize({ width: 2162, height: 1606 });
  const yaw145State = await capture(page, "145", "98.4", true, CULL_YAW_145_SCREENSHOT_PATH);
  const yaw144State = await capture(page, "144", "98.0", false, CULL_YAW_144_SCREENSHOT_PATH);

  expect(yaw145State.error).toBeUndefined();
  expect(yaw144State.error).toBeUndefined();
  expect(yaw145State.loadedChunkCount).toBeGreaterThan(0);
  expect(yaw144State.loadedChunkCount).toBeGreaterThan(0);
  expect(yaw145State.renderQueueStats?.renderedChunkCount ?? 0).toBeGreaterThan(0);
  expect(yaw144State.renderQueueStats?.renderedChunkCount ?? 0).toBeGreaterThan(0);
});
